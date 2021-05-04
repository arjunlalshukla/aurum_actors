#![allow(dead_code)]
use async_trait::async_trait;
use aurum::cluster::crdt::{
  CausalCmd, CausalDisperse, CausalIntraMsg, CausalMsg, DeltaMutator,
  DispersalPreference, CRDT,
};
use aurum::cluster::{Cluster, ClusterCmd, ClusterConfig, HBRConfig};
use aurum::core::{Actor, ActorContext, Host, LocalRef, Node, Socket};
use aurum::testkit::FailureConfigMap;
use aurum::{unify, AurumInterface};
use im;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write;
use std::thread;
use std::time::Duration;
use tokio::time::sleep;
use CoordinatorMsg::*;

unify!(CRDTTestType =
  CausalIntraMsg<LocalGCounter> |
  CausalMsg<LocalGCounter> |
  CoordinatorMsg |
  DataReceiverMsg
);

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
struct Increment {
  port: u16,
}
impl DeltaMutator<LocalGCounter> for Increment {
  fn apply(&self, target: &LocalGCounter) -> LocalGCounter {
    let mut ret = target.clone();
    if let Some(cnt) = ret.map.get_mut(&self.port) {
      *cnt += 1;
    } else {
      ret.map.insert(self.port, 1);
    }
    ret
  }
}

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
struct LocalGCounter {
  pub map: im::OrdMap<u16, u64>,
}
impl CRDT for LocalGCounter {
  type Delta = Increment;

  fn delta(&self, changes: &Self::Delta) -> Self {
    changes.apply(self)
  }

  fn empty(&self) -> bool {
    self.map.is_empty()
  }

  fn join(self, other: Self) -> Self {
    Self {
      map: self.map.union_with(other.map, std::cmp::max),
    }
  }

  fn minimum() -> Self {
    Self {
      map: im::OrdMap::new(),
    }
  }
}

struct TestNode {
  node: Node<CRDTTestType>,
  cluster: LocalRef<ClusterCmd>,
  counter: LocalRef<CausalCmd<LocalGCounter>>,
  recvr: LocalRef<DataReceiverMsg>,
  view: Option<LocalGCounter>,
}

#[derive(AurumInterface, Serialize, Deserialize)]
enum CoordinatorMsg {
  Data(u16, LocalGCounter),
  Mutate(Increment),
}

struct Coordinator {
  clr_cfg: ClusterConfig,
  hbr_cfg: HBRConfig,
  fail_map: FailureConfigMap,
  preference: DispersalPreference,
  ports: Vec<u16>,
  nodes: BTreeMap<u16, TestNode>,
}
#[async_trait]
impl Actor<CRDTTestType, CoordinatorMsg> for Coordinator {
  async fn pre_start(
    &mut self,
    ctx: &ActorContext<CRDTTestType, CoordinatorMsg>,
  ) {
    for port in self.ports.iter().cloned() {
      let socket = Socket::new(Host::DNS("127.0.0.1".to_string()), port, 0);
      let node = Node::<CRDTTestType>::new(socket.clone(), 1).unwrap();
      let cluster = Cluster::new(
        &node,
        "test-crdt-cluster".to_string(),
        3,
        vec![],
        self.fail_map.clone(),
        self.clr_cfg.clone(),
        self.hbr_cfg.clone(),
      )
      .await;
      let counter = CausalDisperse::new(
        &node,
        "test-crdt-causal".to_string(),
        self.fail_map.clone(),
        vec![],
        self.preference.clone(),
        cluster.clone(),
      );
      let recvr = DataReceiver {
        coor: ctx.local_interface(),
        data: counter.clone(),
      };
      let recvr = node.spawn(false, recvr, "".to_string(), false).local().clone().unwrap();
      counter.send(CausalCmd::Subscribe(recvr.transform()));
      let entry = TestNode {
        node: node,
        cluster: cluster,
        counter: counter,
        recvr: recvr,
        view: None,
      };
      self.nodes.insert(port, entry);
    }
  }

  async fn recv(
    &mut self,
    _: &ActorContext<CRDTTestType, CoordinatorMsg>,
    msg: CoordinatorMsg,
  ) {
    match msg {
      Data(port, data) => {
        let test = self.nodes.get_mut(&port).unwrap();
        test.view = Some(data);
        // use one string to ensure atomic printing
        let mut print = String::new();
        writeln!(&mut print, "Got data, printing views").unwrap();
        for (port, node) in self.nodes.iter() {
          if let Some(view) = &node.view {
            writeln!(&mut print, "Port {}", port).unwrap();
            for (p, c) in view.map.iter() {
              writeln!(&mut print, "{} -> {}", p, c).unwrap();
            }
          } else {
            writeln!(&mut print, "Port {} - No Entry", port).unwrap();
          }
        }
        println!("{}", print);
      }
      Mutate(mutator) => {
        self
          .nodes
          .get(&mutator.port)
          .unwrap()
          .recvr
          .send(DataReceiverMsg::Mutate(mutator));
      }
    }
  }
}

#[derive(AurumInterface, Serialize, Deserialize)]
enum DataReceiverMsg {
  #[aurum(local)]
  Data(LocalGCounter),
  Mutate(Increment),
}
struct DataReceiver {
  coor: LocalRef<CoordinatorMsg>,
  data: LocalRef<CausalCmd<LocalGCounter>>,
}
#[async_trait]
impl Actor<CRDTTestType, DataReceiverMsg> for DataReceiver {
  async fn recv(
    &mut self,
    ctx: &ActorContext<CRDTTestType, DataReceiverMsg>,
    msg: DataReceiverMsg,
  ) {
    match msg {
      DataReceiverMsg::Data(counter) => {
        self.coor.send(Data(ctx.node.socket().udp, counter));
      }
      DataReceiverMsg::Mutate(m) => {
        self.data.send(CausalCmd::Mutate(m));
      }
    }
  }
}

//#[test]
fn crdt_test() {
  let mut clr = ClusterConfig::default();
  clr.ping_timeout = Duration::from_millis(50);
  clr.num_pings = 20;
  let hbr = HBRConfig::default();
  let fail_map = FailureConfigMap::default();
  let preference = DispersalPreference::default();
  let socket = Socket::new(Host::DNS("127.0.0.1".to_string()), 5500, 0);
  let node = Node::<CRDTTestType>::new(socket.clone(), 1).unwrap();
  let ports = vec![5501, 5502, 5503, 5504];
  let actor = Coordinator {
    clr_cfg: clr,
    hbr_cfg: hbr,
    fail_map: fail_map,
    preference: preference,
    ports: ports,
    nodes: BTreeMap::new(),
  };
  let coor = node
    .spawn(false, actor, "".to_string(), false)
    .local()
    .clone()
    .unwrap();
  let millis = Duration::from_millis(200);
  let events = vec![
    (Mutate(Increment { port: 5501 }), millis),
    (Mutate(Increment { port: 5502 }), millis),
    (Mutate(Increment { port: 5503 }), millis),
    (Mutate(Increment { port: 5504 }), millis),
    (Mutate(Increment { port: 5501 }), millis),
    (Mutate(Increment { port: 5502 }), millis),
  ];
  node.rt().spawn(execute_events(coor, events));
  thread::sleep(Duration::from_secs(5000));
}

async fn execute_events(
  coor: LocalRef<CoordinatorMsg>,
  vec: Vec<(CoordinatorMsg, Duration)>,
) {
  for (msg, dur) in vec.into_iter() {
    sleep(dur).await;
    coor.send(msg);
  }
}
