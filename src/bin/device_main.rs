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
use rand::{Rng, SeedableRng};
use rand::rngs::SmallRng;
use rpds::RedBlackTreeMapSync;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{channel, Sender};

unify!(
  BenchmarkTypes = DataCenterBusinessMsg
    | ReportReceiverMsg
    | IoTBusinessMsg
    | CollectorMsg
    | PeriodicKillerMsg
);

// in milliseconds
struct KillerDelays {
  min_kill: u64,
  max_kill: u64,
  min_restart: u64,
  max_restart: u64
}

const FAILURE_MODE: FailureMode = FailureMode::None;
const LOG_LEVEL: LogLevel = LogLevel::Info;
const CLUSTER_NAME: &'static str = "my-cool-device-cluster";

fn main() {
  // Exclude the command
  let mut args = std::env::args().skip(1);
  let host = args.next().unwrap();
  let port = args.next().unwrap().parse().unwrap();
  let mode = args.next().unwrap();
  let socket = Socket::new(Host::DNS(host), port, 1001);
  println!("Starting {} on {}", mode, socket);
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
  clr_cfg.vnodes = 20;
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
  _: &mut impl Iterator<Item = String>,
) {
  let mut args = std::env::args(); 
  let bin = args.next().unwrap();
  let host = args.next().unwrap();
  let port: u16 = args.next().unwrap().parse().unwrap();
  // the mode
  args.next().unwrap();
  let min_kill = Duration::from_millis(args.next().unwrap().parse().unwrap());
  let max_kill = Duration::from_millis(args.next().unwrap().parse().unwrap());
  let min_restart = Duration::from_millis(args.next().unwrap().parse().unwrap());
  let max_restart = Duration::from_millis(args.next().unwrap().parse().unwrap());
  let num_procs: u16 = args.next().unwrap().parse().unwrap();
  let args = args.collect_vec();
  let mut cmds = BTreeMap::new();
  for i in 0..num_procs {
    let p = port + i + 1;
    let mut cmd = Command::new(&bin);
    cmd.arg(&host);
    cmd.arg(p.to_string());
    for s in &args {
      cmd.arg(s);
    }
    cmds.insert(p, cmd);
  }
  let actor = PeriodicKiller {
    notify: notify,
    cmds,
    procs: BTreeMap::new(),
    min_kill: min_kill,
    max_kill: max_kill,
    min_restart: min_restart,
    max_restart: max_restart,
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
  let num_clients = first[0].parse().unwrap();
  let print_int = Duration::from_millis(first[1].parse().unwrap());
  let req_int = Duration::from_millis(first[2].parse().unwrap());
  let server_line = lines.next().unwrap().unwrap();
  let server_line = server_line.split_whitespace().collect_vec();
  let server_delays = KillerDelays {
    min_kill: server_line[0].parse().unwrap(),
    max_kill: server_line[1].parse().unwrap(),
    min_restart: server_line[2].parse().unwrap(),
    max_restart: server_line[3].parse().unwrap(),
  };
  let client_line = lines.next().unwrap().unwrap();
  let client_line = client_line.split_whitespace().collect_vec();
  let client_delays = KillerDelays {
    min_kill: client_line[0].parse().unwrap(),
    max_kill: client_line[1].parse().unwrap(),
    min_restart: client_line[2].parse().unwrap(),
    max_restart: client_line[3].parse().unwrap(),
  };
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
    clients: clients.into_iter().map(|((host, port), _)| (host, port)).collect_vec(),
    clients_per_node: num_clients,
    ssh_procs: Vec::new(),
    collection: BTreeMap::new(),
    print_int: print_int,
    req_int: req_int,
    start: Instant::now(),
    prev_total: 0,
    req_since_display: 0,
    server_delays: server_delays,
    client_delays: client_delays,
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
    let log = format!("ReportReceiver for {}", charge.socket);
    info!(LOG_LEVEL, node, log);
    let name = format!("report-recvr-{}", charge.socket);
    let actor = Self {
      supervisor: supervisor,
      charge: charge,
      charge_dest: Destination::new::<IoTBusinessMsg>(CLUSTER_NAME.to_string()),
      req_timeout: req_interval,
      reqs_sent: 0,
      reqs_recvd: 0,
      fail_map: fail_map,
    };
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
  clients: Vec<(String, u16)>,
  clients_per_node: u16,
  ssh_procs: Vec<Child>,
  collection: BTreeMap<Socket, RedBlackTreeMapSync<Device, u64>>,
  print_int: Duration,
  req_int: Duration,
  start: Instant,
  prev_total: u64,
  req_since_display: u64,
  server_delays: KillerDelays,
  client_delays: KillerDelays,
}
impl Collector {
  fn ssh_cmds(&self, first: bool, is_svr: bool, host: &String, port: u16) -> Child {
    let bin = std::env::args().next().unwrap();
    let dir = std::env::current_dir().unwrap().to_str().unwrap().to_string();
    let mut s = String::new();
    write!(s, "cd {}; ", dir).unwrap();
    write!(s, "{} {} {} killer ", bin, host, port).unwrap();
    if is_svr {
      write!(s, " {} {} {} {} 1 server 0.0 0 0 200 ",
        self.server_delays.min_kill,
        self.server_delays.max_kill,
        self.server_delays.min_restart,
        self.server_delays.max_restart,
      ).unwrap();
    } else {
      write!(s, " {} {} {} {} {} client 0.0 0 0 ",
        self.client_delays.min_kill,
        self.client_delays.max_kill,
        self.client_delays.min_restart,
        self.client_delays.max_restart,
        self.clients_per_node,
      ).unwrap();
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
    for (h, p) in &self.clients {
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
        self.req_since_display += 1;
      }
      CollectorMsg::PrintTick => {
        let mut s = String::new();
        let mut total = 0;
        for (_, map) in &self.collection {
          for (_, count) in map {
            //writeln!(s, "  {} | {} -> {}", socket, device.socket, count).unwrap();
            total += *count;
          }
        }
        write!(s, "").unwrap();
        let elapsed = self.start.elapsed();
        let rate = total as f64 / elapsed.as_secs_f64();
        let since_last = total - self.prev_total;
        let current_rate = since_last as f64 / self.print_int.as_secs_f64();
        println!("Elapsed: {:#?}; Total: {}; Rate: {}; Since last print: {}; Current Rate: {}; Since Display: {}\n{}",
          elapsed, total, rate, since_last, current_rate, self.req_since_display, s
        );
        self.prev_total = total;
        self.req_since_display = 0;
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
  Kill(u16),
  Spawn(u16),
  Done
}
struct PeriodicKiller {
  notify: Sender<()>,
  cmds: BTreeMap<u16, Command>,
  procs: BTreeMap<u16, Child>,
  min_kill: Duration,
  max_kill: Duration,
  min_restart: Duration,
  max_restart: Duration,
}
#[async_trait]
impl Actor<BenchmarkTypes, PeriodicKillerMsg> for PeriodicKiller {
  async fn pre_start(&mut self, ctx: &ActorContext<BenchmarkTypes, PeriodicKillerMsg>) {
    for (port, cmd) in self.cmds.iter_mut() {
      self.procs.insert(*port, cmd.spawn().unwrap());
      ctx.node.schedule_local_msg(
        random_duration(self.min_kill, self.max_kill), 
        ctx.local_interface(),
        PeriodicKillerMsg::Kill(*port) 
      );
      info!(LOG_LEVEL, &ctx.node, format!("Spawning port {}", port));
    }
  }

  async fn recv(
    &mut self,
    ctx: &ActorContext<BenchmarkTypes, PeriodicKillerMsg>,
    msg: PeriodicKillerMsg,
  ) {
    match msg {
      PeriodicKillerMsg::Kill(port) => {
        let mut proc = self.procs.remove(&port).unwrap();
        proc.kill().unwrap();
        ctx.node.schedule_local_msg(
          random_duration(self.min_restart, self.max_restart), 
          ctx.local_interface(),
          PeriodicKillerMsg::Spawn(port) 
        );
        info!(LOG_LEVEL, &ctx.node, format!("Killing port {}", port));
      }
      PeriodicKillerMsg::Spawn(port) => {
        let proc = self.cmds.get_mut(&port).unwrap().spawn().unwrap();
        self.procs.insert(port, proc);
        ctx.node.schedule_local_msg(
          random_duration(self.min_kill, self.max_kill), 
          ctx.local_interface(),
          PeriodicKillerMsg::Kill(port) 
        );
        info!(LOG_LEVEL, &ctx.node, format!("Spawning port {}", port));
      }
      PeriodicKillerMsg::Done => {
        for (_, p) in &mut self.procs {
          p.kill().unwrap();
        }
        self.notify.send(()).await.unwrap();
      }
    }
  }
}

fn random_duration(min: Duration, max: Duration) -> Duration {
  let range = min.as_millis()..=max.as_millis();
  Duration::from_millis(SmallRng::from_entropy().gen_range(range) as u64)
}