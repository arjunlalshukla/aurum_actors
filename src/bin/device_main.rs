use async_trait::async_trait;
use aurum::cluster::devices::{
  Charges, Device, DeviceClient, DeviceClientConfig, DeviceServer,
  DeviceServerCmd,
};
use aurum::cluster::{Cluster, ClusterConfig, HBRConfig};
use aurum::core::{
  udp_msg, Actor, ActorContext, ActorRef, ActorSignal, Destination, Host, LocalRef,
  Node, Socket,
};
use aurum::testkit::{FailureConfigMap, FailureMode, LogLevel};
use aurum::{info, udp_select, unify, AurumInterface};
use itertools::Itertools;
use rpds::RedBlackTreeMapSync;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command};
use std::time::Duration;
use tokio::sync::mpsc::{channel, Sender};

unify!(
  BenchmarkTypes = DataCenterBusinessMsg
    | ReportReceiverMsg
    | IoTBusinessMsg
    | CollectorMsg
    | PeriodicKillerMsg
);

const FAILURE_MODE: FailureMode = FailureMode::None;
const LOG_LEVEL: LogLevel = LogLevel::Warn;
const CLUSTER_NAME: &'static str = "my-cool-device-cluster";

fn main() {
  // Exclude the command
  let mut args = std::env::args().skip(1);
  let host = args.next().unwrap();
  let port = args.next().unwrap().parse().unwrap();
  let mode = args.next().unwrap();
  let socket = Socket::new(Host::DNS(host), port, 1001);
  let node = Node::<BenchmarkTypes>::new(socket, num_cpus::get()).unwrap();
  let (tx, mut rx) = channel(1);

  match mode.as_str() {
    "server" => server(tx, &node, &mut args),
    "client" => client(tx, &node, &mut args),
    "killer" => killer(tx, &node, &mut args),
    "collector" => collector(tx, &node, &mut args),
    _ => panic!("invalid mode {}", mode),
  }

  node.rt().block_on(rx.recv());
}

fn get_fail_map(args: &mut impl Iterator<Item = String>) -> FailureConfigMap {
  let mut fail_map = FailureConfigMap::default();
  fail_map.cluster_wide.drop_prob = args.next().unwrap().parse().unwrap();
  let lower = args.next().unwrap().parse().unwrap();
  let upper = args.next().unwrap().parse().unwrap();
  fail_map.cluster_wide.delay = if upper == 0 && lower == 0 {
    None
  } else {
    Some((Duration::from_millis(lower), Duration::from_millis(upper)))
  };
  fail_map
}

fn get_seeds(args: &mut impl Iterator<Item = String>, delim: Option<&str>) -> Vec<Socket> {
  let mut args = args.peekable();
  let mut seeds = Vec::new();
  while args.peek().map(|x| x.as_str()) != delim {
    seeds.push(Socket::new(
      Host::DNS(args.next().unwrap()),
      args.next().unwrap().parse().unwrap(),
      0,
    ));
  }
  seeds
}

fn server(
  notify: Sender<()>,
  node: &Node<BenchmarkTypes>,
  args: &mut impl Iterator<Item = String>,
) {
  let fail_map = get_fail_map(args);

  let interval = Duration::from_millis(args.next().unwrap().parse().unwrap());

  let name = CLUSTER_NAME.to_string();
  let mut clr_cfg = ClusterConfig::default();
  clr_cfg.seed_nodes = get_seeds(args, None);
  let hbr_cfg = HBRConfig::default();

  let cluster = Cluster::new(
    &node,
    name.clone(),
    vec![],
    fail_map.clone(),
    clr_cfg,
    hbr_cfg,
  );
  let f = fail_map.clone();
  let devices = DeviceServer::new(&node, cluster, vec![], name.clone(), f);
  let business = DataCenterBusiness {
    notify: notify,
    report_interval: interval,
    devices: devices,
    charges: BTreeMap::new(),
    totals: RedBlackTreeMapSync::new_sync(),
    fail_map: fail_map,
  };
  node.spawn(false, business, name, true);
}

fn client(
  notify: Sender<()>,
  node: &Node<BenchmarkTypes>,
  args: &mut impl Iterator<Item = String>,
) {
  let fail_map = get_fail_map(args);

  let name = CLUSTER_NAME.to_string();
  let mut cfg = DeviceClientConfig::default();
  cfg.seeds = get_seeds(args, None).into_iter().collect();

  DeviceClient::new(&node, cfg, name.clone(), fail_map.clone(), vec![]);
  let actor = IoTBusiness { notify: notify, fail_map: fail_map };
  node.spawn(false, actor, name, true);
}

fn killer(
  notify: Sender<()>,
  node: &Node<BenchmarkTypes>,
  args: &mut impl Iterator<Item = String>,
) {
  let bin = std::env::args().next().unwrap();
  let mut cmd = Command::new(bin);
  args.for_each(|s| {cmd.arg(s);});
  let actor = PeriodicKiller {
    notify: notify,
    cmd: cmd,
    proc: None,
  };
  node.spawn(false, actor, CLUSTER_NAME.to_string(), true);
}

fn collector(
  notify: Sender<()>,
  node: &Node<BenchmarkTypes>,
  args: &mut impl Iterator<Item = String>,
) {
  let _bin = std::env::args().next().unwrap();
  let file = File::open(args.next().unwrap()).unwrap();
  let mut lines = BufReader::new(file).lines();
  let mut servers = BTreeMap::new();
  let mut clients = BTreeMap::new();
  let first = lines.next().unwrap().unwrap();
  let first = first.split_whitespace().collect_vec();
  let print_int = Duration::from_millis(first[0].parse().unwrap());
  let req_int = Duration::from_millis(first[1].parse().unwrap());
  for (num, line) in lines.enumerate().map(|(n, l)| (n + 1, l.unwrap())) {
    let toks = line.split_whitespace().collect_vec();
    if toks.len() != 3 {
      panic!("Err on line {}, must have 3 tokens", num);
    }
    let socket = (toks[1].to_string(), toks[2].to_string().parse().unwrap());
    if socket.1 %2 != 0 {
      panic!("Line {} port must be even", num);
    }
    match toks[0] {
      "server" => {
        let svr = servers.get(&socket);
        let cli = clients.get(&socket);
        if let Some(line_num) = svr {
          panic!("Tried server on line {}, but is server on line {}", num, line_num);
        } else if let Some(line_num) = cli {
          panic!("Tried server on line {}, but is client on line {}", num, line_num);
        } else {
          servers.insert(socket, num);
        }
      }
      "client" => {
        let svr = servers.get(&socket);
        let cli = clients.get(&socket);
        if let Some(line_num) = svr {
          panic!("Tried client on line {}, but is server on line {}", num, line_num);
        } else if let Some(line_num) = cli {
          panic!("Tried client on line {}, but is client on line {}", num, line_num);
        } else {
          clients.insert(socket, num);
        }
      }
      a => panic!("Invalid node option {}", a),
    }
  }
  let actor = Collector {
    notify: notify,
    _kill_dest: Destination::new::<PeriodicKillerMsg>(CLUSTER_NAME.to_string()),
    svr_dest: Destination::new::<DataCenterBusinessMsg>(CLUSTER_NAME.to_string()),
    servers: servers.into_iter().map(|((host, port), _)| {
      let socket = Socket::new(Host::DNS(host.clone()), port + 1, 0);
      (host, port, socket)
    }).collect_vec(),
    clients: clients.into_iter().map(|((host, port), _)| {
      let socket = Socket::new(Host::DNS(host.clone()), port + 1, 0);
      (host, port, socket)
    }).collect_vec(),
    ssh_procs: Vec::new(),
    collection: BTreeMap::new(),
    print_int: print_int,
    req_int: req_int,
  };
  node.spawn(false, actor, CLUSTER_NAME.to_string(), true);
}

#[derive(AurumInterface, Serialize, Deserialize)]
enum DataCenterBusinessMsg {
  #[aurum(local)]
  Devices(Charges),
  Report(Device, u128, u64, u64),
  ReportReq(ActorRef<BenchmarkTypes, CollectorMsg>),
}
struct DataCenterBusiness {
  notify: Sender<()>,
  report_interval: Duration,
  devices: LocalRef<DeviceServerCmd>,
  charges: BTreeMap<Device, LocalRef<ReportReceiverMsg>>,
  totals: RedBlackTreeMapSync<Device, u64>,
  fail_map: FailureConfigMap,
}
#[async_trait]
impl Actor<BenchmarkTypes, DataCenterBusinessMsg> for DataCenterBusiness {
  async fn pre_start(
    &mut self,
    ctx: &ActorContext<BenchmarkTypes, DataCenterBusinessMsg>,
  ) {
    self
      .devices
      .send(DeviceServerCmd::Subscribe(ctx.local_interface()));
  }

  async fn recv(
    &mut self,
    ctx: &ActorContext<BenchmarkTypes, DataCenterBusinessMsg>,
    msg: DataCenterBusinessMsg,
  ) {
    match msg {
      DataCenterBusinessMsg::Devices(Charges(devices, _)) => {
        let mut new_charges = BTreeMap::new();
        for device in devices.into_iter() {
          let (d, r) = self.charges.remove_entry(&device).unwrap_or_else(|| {
            let recvr = ReportReceiver::new(
              &ctx.node,
              ctx.local_interface(),
              device.clone(),
              self.report_interval,
              self.fail_map.clone(),
            );
            (device, recvr)
          });
          new_charges.insert(d, r);
        }
        self.charges.values().for_each(|r| {
          r.signal(ActorSignal::Term);
        });
        self.charges = new_charges;
      }
      DataCenterBusinessMsg::Report(device, _, _, recvs) => {
        let port = device.socket.udp;
        self.totals.insert_mut(device, recvs);
        let log = format!("Received {} reports from {}", recvs, port);
        info!(LOG_LEVEL, &ctx.node, log);
      }
      DataCenterBusinessMsg::ReportReq(r) => {
        let msg = CollectorMsg::Report(ctx.node.socket().clone(), self.totals.clone());
        r.remote_send(&msg).await;
      }
    }
  }

  async fn post_stop(
    &mut self,
    _: &ActorContext<BenchmarkTypes, DataCenterBusinessMsg>,
  ) {
    self.charges.values().for_each(|r| {
      r.signal(ActorSignal::Term);
    });
    self.notify.send(()).await.unwrap();
  }
}

#[derive(AurumInterface, Serialize, Deserialize)]
enum ReportReceiverMsg {
  Tick(u64),
  Report(Vec<u64>),
}
struct ReportReceiver {
  supervisor: LocalRef<DataCenterBusinessMsg>,
  charge: Device,
  charge_dest: Destination<BenchmarkTypes, IoTBusinessMsg>,
  req_timeout: Duration,
  reqs_sent: u64,
  reqs_recvd: u64,
  fail_map: FailureConfigMap,
}
impl ReportReceiver {
  fn new(
    node: &Node<BenchmarkTypes>,
    supervisor: LocalRef<DataCenterBusinessMsg>,
    charge: Device,
    req_interval: Duration,
    fail_map: FailureConfigMap,
  ) -> LocalRef<ReportReceiverMsg> {
    let actor = Self {
      supervisor: supervisor,
      charge: charge,
      charge_dest: Destination::new::<IoTBusinessMsg>(CLUSTER_NAME.to_string()),
      req_timeout: req_interval,
      reqs_sent: 0,
      reqs_recvd: 0,
      fail_map: fail_map,
    };
    let name = format!("report-recvr-{}", rand::random::<u64>());
    println!("Starting ReportReceiver with name: {}", name);
    node
      .spawn(false, actor, name, true)
      .local()
      .clone()
      .unwrap()
  }

  async fn req(&self, num: u64, ctx: &ActorContext<BenchmarkTypes, ReportReceiverMsg>) {
    let msg = IoTBusinessMsg::ReportReq(ctx.interface());
    udp_select!(
      FAILURE_MODE,
      &ctx.node,
      &self.fail_map,
      &self.charge.socket,
      &self.charge_dest,
      &msg
    );
    ctx.node.schedule_local_msg(
      self.req_timeout,
      ctx.local_interface(),
      ReportReceiverMsg::Tick(num),
    ); 
  }
}
#[async_trait]
impl Actor<BenchmarkTypes, ReportReceiverMsg> for ReportReceiver {
  async fn pre_start(
    &mut self,
    ctx: &ActorContext<BenchmarkTypes, ReportReceiverMsg>,
  ) {
    self.reqs_sent = 1;
    self.reqs_recvd = 0;
    self.req(self.reqs_sent, ctx).await;
  }

  async fn recv(
    &mut self,
    ctx: &ActorContext<BenchmarkTypes, ReportReceiverMsg>,
    msg: ReportReceiverMsg,
  ) {
    match msg {
      ReportReceiverMsg::Tick(num) => {
        if self.reqs_recvd < num {
          self.req(num, ctx).await;
        } 
      }
      ReportReceiverMsg::Report(contents) => {
        self.reqs_recvd += 1;
        let msg = DataCenterBusinessMsg::Report(
          self.charge.clone(),
          contents.into_iter().map(|x| x as u128).sum(),
          self.reqs_sent,
          self.reqs_recvd,
        );
        self.supervisor.send(msg);
        self.reqs_sent += 1;
        self.req(self.reqs_sent, ctx).await;
      }
    }
  }
}

#[derive(AurumInterface, Serialize, Deserialize)]
enum IoTBusinessMsg {
  ReportReq(ActorRef<BenchmarkTypes, ReportReceiverMsg>),
}
struct IoTBusiness {
  notify: Sender<()>,
  fail_map: FailureConfigMap,
}
#[async_trait]
impl Actor<BenchmarkTypes, IoTBusinessMsg> for IoTBusiness {
  async fn recv(
    &mut self,
    ctx: &ActorContext<BenchmarkTypes, IoTBusinessMsg>,
    msg: IoTBusinessMsg,
  ) {
    match msg {
      IoTBusinessMsg::ReportReq(requester) => {
        let items = (1..1000u64).collect_vec();
        let msg = ReportReceiverMsg::Report(items);
        udp_select!(
          FAILURE_MODE,
          &ctx.node,
          &self.fail_map,
          &requester.socket,
          &requester.dest,
          &msg
        );
      }
    }
  }

  async fn post_stop(&mut self, _: &ActorContext<BenchmarkTypes, IoTBusinessMsg>) {
    self.notify.send(()).await.unwrap();
  }
}

#[derive(AurumInterface, Serialize, Deserialize)]
enum CollectorMsg {
  Report(Socket, RedBlackTreeMapSync<Device, u64>),
  PrintTick,
  ReqTick
}
struct Collector {
  notify: Sender<()>,
  _kill_dest: Destination<BenchmarkTypes, PeriodicKillerMsg>, 
  svr_dest: Destination<BenchmarkTypes, DataCenterBusinessMsg>,
  servers: Vec<(String, u16, Socket)>,
  clients: Vec<(String, u16, Socket)>,
  ssh_procs: Vec<Child>,
  collection: BTreeMap<Socket, RedBlackTreeMapSync<Device, u64>>,
  print_int: Duration,
  req_int: Duration,
}
impl Collector {
  fn ssh_cmds(&self, first: bool, is_svr: bool, host: &String, port: u16) -> Child {
    let bin = std::env::args().next().unwrap();
    let dir = std::env::current_dir().unwrap().to_str().unwrap().to_string();
    let mut s = String::new();
    write!(s, "cd {}; ", dir).unwrap();
    if host.as_str() != "localhost" {
      write!(s, "pkill -f {}; ", bin).unwrap();
      let mut cmd = Command::new("ssh");
      cmd.arg(host);
      cmd.arg(s.clone());
      assert!(cmd.status().unwrap().success());
    }
    write!(s, "{} {} {} killer {} {} ", bin, host, port, host, port + 1).unwrap();
    if is_svr {
      write!(s, " server 0.0 0 0 200 ").unwrap();
    } else {
      write!(s, " client 0.0 0 0 ").unwrap();
    }
    if !first {
      for (h, p, _) in self.servers.iter().filter(|(h, p, _)| h != host || *p != port) {
        write!(s, " {} {} ", h, p + 1).unwrap();
      }  
    }
    println!("Running command ssh {} \"{}\"", host, s);
    let mut cmd = Command::new("ssh");
    cmd.arg(host);
    cmd.arg(s);
    cmd.spawn().unwrap()
  }
}
#[async_trait]
impl Actor<BenchmarkTypes, CollectorMsg> for Collector {
  async fn pre_start(&mut self, ctx: &ActorContext<BenchmarkTypes, CollectorMsg>) {
    let mut first = true;
    for (h, p, _) in &self.servers {
      self.ssh_procs.push(self.ssh_cmds(first, true, h, *p));
      first = false;
    }
    for (h, p, _) in &self.clients {
      self.ssh_procs.push(self.ssh_cmds(false, false, h, *p));
    }
    ctx.local_interface().send(CollectorMsg::PrintTick);
    ctx.local_interface().send(CollectorMsg::ReqTick);
  }

  async fn recv(
    &mut self,
    ctx: &ActorContext<BenchmarkTypes, CollectorMsg>,
    msg: CollectorMsg,
  ) {
    match msg {
      CollectorMsg::Report(from, report) => {
        self.collection.insert(from, report);
      }
      CollectorMsg::PrintTick => {
        let mut s = String::new();
        let mut total = 0;
        for (socket, map) in &self.collection {
          for (device, count) in map {
            writeln!(s, "  {:?}::{} | {:?}::{} -> {}", 
              socket.host, 
              socket.udp, 
              device.socket.host, 
              device.socket.udp, 
              count
            )
            .unwrap();
            total += *count;
          }
        }
        println!("Total: {}\n{}", total, s);
        ctx.node.schedule_local_msg(self.print_int, ctx.local_interface(), CollectorMsg::PrintTick);
      }
      CollectorMsg::ReqTick => {
        let msg = DataCenterBusinessMsg::ReportReq(ctx.interface());
        for socket in &self.servers {
          udp_msg(&socket.2, &self.svr_dest, &msg).await;
        }
        ctx.node.schedule_local_msg(self.req_int, ctx.local_interface(), CollectorMsg::ReqTick);
      }
    }
  }

  async fn post_stop(&mut self, _: &ActorContext<BenchmarkTypes, CollectorMsg>) {
    for proc in &mut self.ssh_procs {
      proc.kill().unwrap();
    }
    self.notify.send(()).await.unwrap();
  }
}

#[allow(dead_code)]
#[derive(AurumInterface)]
#[aurum(local)]
enum PeriodicKillerMsg {
  Spawn,
  Kill,
  Done
}
struct PeriodicKiller {
  notify: Sender<()>,
  cmd: Command,
  proc: Option<Child>,
}
#[async_trait]
impl Actor<BenchmarkTypes, PeriodicKillerMsg> for PeriodicKiller {
  async fn pre_start(&mut self, _: &ActorContext<BenchmarkTypes, PeriodicKillerMsg>) {
    self.proc = Some(self.cmd.spawn().unwrap());
  }

  async fn recv(
    &mut self,
    _ctx: &ActorContext<BenchmarkTypes, PeriodicKillerMsg>,
    msg: PeriodicKillerMsg,
  ) {
    match msg {
      PeriodicKillerMsg::Kill => {
        if let Some(mut p) = self.proc.take() {
          p.kill().unwrap();
        }
      }
      PeriodicKillerMsg::Spawn => {
        if self.proc.is_none() {
          self.proc.replace(self.cmd.spawn().unwrap());
        }
      }
      PeriodicKillerMsg::Done => {
        self.notify.send(()).await.unwrap();
      }
    }
  }
}
