#![allow(dead_code, unused_imports, unused_variables)]
use async_trait::async_trait;
use aurum::cluster::{Cluster, ClusterConfig, HBRConfig};
use aurum::core::{
  Actor, ActorContext, ActorRef, ActorSignal, Destination, Host, LocalRef, Node, Socket,
};
use aurum::testkit::{FailureConfigMap, FailureMode, LogLevel};
use aurum::cluster::devices::{
  Charges, Device, DeviceClient, DeviceClientConfig, DeviceServer, DeviceServerCmd,
};
use aurum::{info, udp_select, unify, AurumInterface};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

unify!(
  BenchmarkTypes = DataCenterBusinessMsg
    | ReportReceiverMsg
    | IoTBusinessMsg
    | CollectorMsg
    | PeriodicKillerMsg
);

const FAILURE_MODE: FailureMode = FailureMode::None;
const LOG_LEVEL: LogLevel = LogLevel::Trace;
const CLUSTER_NAME: &'static str = "my-cool-device-cluster";

fn main() {
  // Exclude the command
  let mut args = std::env::args().skip(1);
  let host = args.next().unwrap();
  let port = args.next().unwrap().parse().unwrap();
  let mode = args.next().unwrap();
  let socket = Socket::new(Host::DNS(host), port, 1001);
  let node = Node::<BenchmarkTypes>::new(socket, 1).unwrap();

  match mode.as_str() {
    "server" => server(node, &mut args),
    "client" => client(node, &mut args),
    "collector" => {}
    "killer" => {}
    _ => panic!("invalid mode {}", mode),
  }

  std::thread::sleep(Duration::from_secs(500));
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

fn get_seeds(args: &mut impl Iterator<Item = String>) -> Vec<Socket> {
  let mut args = args.peekable();
  let mut seeds = Vec::new();
  while args.peek().is_some() {
    seeds.push(Socket::new(
      Host::DNS(args.next().unwrap()),
      args.next().unwrap().parse().unwrap(),
      0
    ));
  }
  seeds
}

fn server(node: Node<BenchmarkTypes>, args: &mut impl Iterator<Item = String>) {
  let fail_map = get_fail_map(args);

  let name = CLUSTER_NAME.to_string();
  let mut clr_cfg = ClusterConfig::default();
  clr_cfg.seed_nodes = get_seeds(args);
  let hbr_cfg = HBRConfig::default();

  let cluster = Cluster::new(
    &node,
    name.clone(),
    vec![],
    fail_map.clone(),
    clr_cfg,
    hbr_cfg,
  );
  DeviceServer::new(&node, cluster, vec![], name, fail_map);
}

fn client(node: Node<BenchmarkTypes>, args: &mut impl Iterator<Item = String>) {
  let fail_map = get_fail_map(args);

  let name = CLUSTER_NAME.to_string();
  let mut cfg = DeviceClientConfig::default();
  cfg.seeds = get_seeds(args).into_iter().collect();

  DeviceClient::new(&node, cfg, name, fail_map, vec![]);
}

#[derive(AurumInterface)]
#[aurum(local)]
enum DataCenterBusinessMsg {
  #[aurum(local)]
  Devices(Charges),
  Report(Device, u128, u64, u64),
  ReportReq(Socket),
}
struct DataCenterBusiness {
  report_interval: Duration,
  devices: LocalRef<DeviceServerCmd>,
  charges: HashMap<Device, LocalRef<ReportReceiverMsg>>,
  totals: HashMap<Device, u128>,
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
        let mut new_charges = HashMap::new();
        for device in devices.into_iter() {
          let d = match self.charges.remove_entry(&device) {
            Some((d, r)) => {
              r.signal(ActorSignal::Term);
              d
            }
            None => device.clone()
          };
          let recvr = ReportReceiver::new(
            &ctx.node,
            ctx.local_interface(),
            device,
            self.report_interval,
            self.fail_map.clone()
          );
          new_charges.insert(d, recvr);
        }
        self.charges.values().for_each(|r| {r.signal(ActorSignal::Term);});
        self.charges = new_charges;
      }
      DataCenterBusinessMsg::Report(device, total, sends, recvs) => {
        let count = self.totals.get_mut(&device).unwrap();
        *count += total;
        info!(
          LOG_LEVEL,
          &ctx.node,
          format!(
            "Got {}, bringing total to {} from {} where sends = {}; recvs = {}",
            total, *count, device.socket.udp, sends, recvs
          )
        )
      }
      DataCenterBusinessMsg::ReportReq(_) => {}
    }
  }

  async fn post_stop(&mut self, ctx: &ActorContext<BenchmarkTypes, DataCenterBusinessMsg>) {
    self.charges.values().for_each(|r| {r.signal(ActorSignal::Term);});
  }
}

#[derive(AurumInterface, Serialize, Deserialize)]
enum ReportReceiverMsg {
  Tick,
  Report(Vec<u64>),
}
struct ReportReceiver {
  supervisor: LocalRef<DataCenterBusinessMsg>,
  charge: Device,
  charge_dest: Destination<BenchmarkTypes, IoTBusinessMsg>,
  req_interval: Duration,
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
      req_interval: req_interval,
      reqs_sent: 0,
      reqs_recvd: 0,
      fail_map: fail_map
    };
    let name = format!("report-recvr-{}", rand::random::<u64>());
    node.spawn(false, actor, name, true).local().clone().unwrap()
  }
}
#[async_trait]
impl Actor<BenchmarkTypes, ReportReceiverMsg> for ReportReceiver {
  async fn recv(
    &mut self,
    ctx: &ActorContext<BenchmarkTypes, ReportReceiverMsg>,
    msg: ReportReceiverMsg,
  ) {
    match msg {
      ReportReceiverMsg::Tick => {
        self.reqs_sent += 1;
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
          self.req_interval,
          ctx.local_interface(),
          ReportReceiverMsg::Tick,
        );
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
      }
    }
  }
}

#[derive(AurumInterface, Serialize, Deserialize)]
enum IoTBusinessMsg {
  ReportReq(ActorRef<BenchmarkTypes, ReportReceiverMsg>),
}
struct IoTBusiness {}
#[async_trait]
impl Actor<BenchmarkTypes, IoTBusinessMsg> for IoTBusiness {
  async fn recv(
    &mut self,
    ctx: &ActorContext<BenchmarkTypes, IoTBusinessMsg>,
    msg: IoTBusinessMsg,
  ) {
    match msg {
      IoTBusinessMsg::ReportReq(_) => {}
    }
  }
}

#[derive(AurumInterface)]
#[aurum(local)]
enum CollectorMsg {}
struct Collector {}
#[async_trait]
impl Actor<BenchmarkTypes, CollectorMsg> for Collector {
  async fn recv(
    &mut self,
    ctx: &ActorContext<BenchmarkTypes, CollectorMsg>,
    msg: CollectorMsg,
  ) {
    match msg {}
  }
}

#[derive(AurumInterface)]
#[aurum(local)]
enum PeriodicKillerMsg {}
struct PeriodicKiller {}
#[async_trait]
impl Actor<BenchmarkTypes, PeriodicKillerMsg> for PeriodicKiller {
  async fn recv(
    &mut self,
    ctx: &ActorContext<BenchmarkTypes, PeriodicKillerMsg>,
    msg: PeriodicKillerMsg,
  ) {
    match msg {}
  }
}
