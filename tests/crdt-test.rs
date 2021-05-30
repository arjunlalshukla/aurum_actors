use async_trait::async_trait;
use aurum::cluster::crdt::{
  CausalCmd, CausalDisperse, CausalIntraMsg, CausalMsg, DeltaMutator,
  DispersalPreference, DispersalSelector, CRDT,
};
use aurum::cluster::{Cluster, ClusterConfig, HBRConfig};
use aurum::core::{Actor, ActorContext, Host, LocalRef, Node, Socket};
use aurum::testkit::FailureConfigMap;
use aurum::{unify, AurumInterface};
use im;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fmt::Write;
use std::time::Duration;
use tokio::sync::mpsc::{channel, Sender};
use CoordinatorMsg::*;

unify!(CRDTTestType =
  CausalMsg<LocalGCounter> |
  CoordinatorMsg |
  DataReceiverMsg
  ;
  CausalIntraMsg<LocalGCounter>
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
  recvr: LocalRef<DataReceiverMsg>,
  view: Option<LocalGCounter>,
}

#[derive(AurumInterface, Serialize, Deserialize)]
enum CoordinatorMsg {
  Data(u16, LocalGCounter),
  Mutate(Increment),
  Spawn(u16),
  WaitForConvergence,
  Done,
}

struct Coordinator {
  clr_cfg: ClusterConfig,
  hbr_cfg: HBRConfig,
  fail_map: FailureConfigMap,
  preference: DispersalPreference,
  nodes: BTreeMap<u16, TestNode>,
  convergence: LocalGCounter,
  converged: HashSet<u16>,
  queue: Vec<CoordinatorMsg>,
  waiting: bool,
  notification: Sender<()>,
}
impl Coordinator {
  fn convergence_reached(&self) -> bool {
    self.nodes.keys().all(|k| self.converged.contains(k))
  }
}
#[async_trait]
impl Actor<CRDTTestType, CoordinatorMsg> for Coordinator {
  async fn recv(
    &mut self,
    ctx: &ActorContext<CRDTTestType, CoordinatorMsg>,
    msg: CoordinatorMsg,
  ) {
    match msg {
      Data(port, data) => {
        if self.waiting && data == self.convergence {
          self.converged.insert(port);
        }
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
        if self.waiting && self.convergence_reached() {
          println!("CONVERGENCE reached!");
          self.waiting = false;
          let my_ref = ctx.local_interface();
          for msg in self.queue.drain(..) {
            my_ref.send(msg);
          }
        }
      }
      Mutate(mutator) => {
        if self.waiting {
          self.queue.push(Mutate(mutator));
          return;
        }
        let d = mutator.apply(&self.convergence);
        if !d.empty() {
          self.convergence = self.convergence.clone().join(d);
          self.converged = HashSet::new();
        }
        self
          .nodes
          .get(&mutator.port)
          .unwrap()
          .recvr
          .send(DataReceiverMsg::Mutate(mutator));
      }
      Spawn(port) => {
        if self.waiting {
          self.queue.push(Spawn(port));
          return;
        }
        let socket = Socket::new(Host::DNS("127.0.0.1".to_string()), port, 0);
        let node = Node::<CRDTTestType>::new(socket.clone(), 1).unwrap();
        let mut clr_cfg = self.clr_cfg.clone();
        clr_cfg.seed_nodes = self
          .nodes
          .keys()
          .take(3)
          .map(|p| Socket::new(Host::DNS("127.0.0.1".to_string()), *p, 0))
          .collect();
        let cluster = Cluster::new(
          &node,
          "test-crdt-cluster".to_string(),
          vec![],
          self.fail_map.clone(),
          clr_cfg,
          self.hbr_cfg.clone(),
        );
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
        let recvr = node
          .spawn(false, recvr, "".to_string(), false)
          .local()
          .clone()
          .unwrap();
        counter.send(CausalCmd::Subscribe(recvr.transform()));
        let entry = TestNode {
          recvr: recvr,
          view: None,
        };
        self.nodes.insert(port, entry);
      }
      WaitForConvergence => {
        if self.waiting {
          self.queue.push(WaitForConvergence);
          return;
        }
        if !self.convergence_reached() {
          println!("Waiting for CONVERGENCE");
          self.waiting = true;
          self.queue.clear();
        } else {
          println!("CONVERGENCE already reached");
        }
      }
      Done => {
        if self.waiting {
          self.queue.push(Done);
          return;
        }
        if self.convergence_reached() {
          println!("Done!");
          self.notification.send(()).await.unwrap();
        } else {
          println!("Waiting for CONVERGENCE");
          self.waiting = true;
          self.queue.clear();
          self.queue.push(Done);
        }
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

fn run_test(
  clr_cfg: ClusterConfig,
  hbr_cfg: HBRConfig,
  fail_map: FailureConfigMap,
  preference: DispersalPreference,
) {
  let socket = Socket::new(Host::DNS("127.0.0.1".to_string()), 5500, 0);
  let node = Node::<CRDTTestType>::new(socket.clone(), 1).unwrap();
  let (tx, mut rx) = channel(1);
  let actor = Coordinator {
    clr_cfg: clr_cfg,
    hbr_cfg: hbr_cfg,
    fail_map: fail_map,
    preference: preference,
    nodes: BTreeMap::new(),
    convergence: LocalGCounter::minimum(),
    converged: HashSet::new(),
    queue: Vec::new(),
    waiting: false,
    notification: tx,
  };
  let coor = node
    .spawn(false, actor, "".to_string(), false)
    .local()
    .clone()
    .unwrap();
  let events = vec![
    Spawn(5501),
    Spawn(5502),
    Spawn(5503),
    Spawn(5504),
    Mutate(Increment { port: 5501 }),
    Mutate(Increment { port: 5502 }),
    Mutate(Increment { port: 5503 }),
    Mutate(Increment { port: 5504 }),
    Mutate(Increment { port: 5501 }),
    Mutate(Increment { port: 5502 }),
    WaitForConvergence,
    Spawn(5505),
    Spawn(5506),
    Mutate(Increment { port: 5505 }),
    Mutate(Increment { port: 5505 }),
    Mutate(Increment { port: 5505 }),
    Mutate(Increment { port: 5506 }),
    Mutate(Increment { port: 5506 }),
    Mutate(Increment { port: 5506 }),
    Done,
  ];
  for e in events {
    coor.send(e);
  }
  let timeout = Duration::from_millis(10_000);
  node.rt().block_on(async {
    tokio::time::timeout(timeout, rx.recv())
      .await
      .unwrap()
      .unwrap()
  });
}

//#[test]
#[allow(dead_code)]
fn crdt_test_out_of_date() {
  let mut clr = ClusterConfig::default();
  clr.ping_timeout = Duration::from_millis(200);
  clr.num_pings = 20;
  clr.vnodes = 3;
  let hbr = HBRConfig::default();
  let mut fail_map = FailureConfigMap::default();
  fail_map.cluster_wide.drop_prob = 0.5;
  fail_map.cluster_wide.delay =
    Some((Duration::from_millis(20), Duration::from_millis(50)));
  let mut preference = DispersalPreference::default();
  preference.timeout = Duration::from_millis(200);
  run_test(clr, hbr, fail_map, preference);
}

#[test]
fn crdt_test_all() {
  let mut clr = ClusterConfig::default();
  clr.ping_timeout = Duration::from_millis(200);
  clr.num_pings = 20;
  clr.vnodes = 3;
  let hbr = HBRConfig::default();
  let mut fail_map = FailureConfigMap::default();
  fail_map.cluster_wide.drop_prob = 0.5;
  fail_map.cluster_wide.delay =
    Some((Duration::from_millis(20), Duration::from_millis(50)));
  let mut preference = DispersalPreference::default();
  preference.selector = DispersalSelector::All;
  preference.timeout = Duration::from_millis(200);
  run_test(clr, hbr, fail_map, preference);
}
