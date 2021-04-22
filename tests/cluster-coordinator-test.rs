use async_trait::async_trait;
use aurum::cluster::{ClusterConfig, ClusterEventSimple, HBRConfig};
use aurum::core::{
  Actor, ActorContext, ActorRef, ActorSignal, Host, Node, Socket,
};
use aurum::test_commons::{ClusterNodeMsg, ClusterNodeTypes, CoordinatorMsg};
use aurum::testkit::FailureConfigMap;
use itertools::Itertools;
use rusty_fork::rusty_fork_test;
use std::collections::HashMap;
use std::process::{Child, Command};
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use CoordinatorMsg::*;

const PORT: u16 = 3000;
const TIMEOUT: Duration = Duration::from_millis(10_000);

#[derive(Clone, Debug)]
enum MemberErrorType {
  Timeout,
  Unexpected,
}

type MemberError = (MemberErrorType, Socket, ClusterEventSimple);

struct ClusterCoor {
  finite: bool,
  nodes:
    HashMap<Socket, (Child, HashMap<ClusterEventSimple, JoinHandle<bool>>)>,
  event_count: usize,
  errors: Vec<MemberError>,
  finish: bool,
  notify: tokio::sync::mpsc::Sender<Vec<MemberError>>,
  fail_map: FailureConfigMap,
  clr_cfg: ClusterConfig,
  hbr_cfg: HBRConfig,
}
impl ClusterCoor {
  fn expect(
    &mut self,
    ctx: &ActorContext<ClusterNodeTypes, CoordinatorMsg>,
    node: Socket,
    event: ClusterEventSimple,
  ) {
    let events = &mut self.nodes.get_mut(&node).unwrap().1;
    let timeout = ctx.node.schedule_local_msg(
      TIMEOUT,
      ctx.local_interface(),
      CoordinatorMsg::TimedOut(node, event.clone()),
    );
    events.insert(event, timeout);
    self.event_count += 1;
  }

  fn event(
    &mut self,
    node: &Socket,
    event: &ClusterEventSimple,
  ) -> Option<JoinHandle<bool>> {
    let ret = self.nodes.get_mut(node).unwrap().1.remove(event);
    if ret.is_some() {
      self.event_count -= 1;
    }
    ret
  }

  fn complete(&self, ctx: &ActorContext<ClusterNodeTypes, CoordinatorMsg>) {
    if self.finish && self.event_count == 0 {
      ctx.local_interface().signal(ActorSignal::Term);
    }
  }
}
#[async_trait]
impl Actor<ClusterNodeTypes, CoordinatorMsg> for ClusterCoor {
  async fn recv(
    &mut self,
    ctx: &ActorContext<ClusterNodeTypes, CoordinatorMsg>,
    msg: CoordinatorMsg,
  ) {
    match msg {
      Up(node) => {
        node
          .move_to(ClusterNodeMsg::FailureMap(
            self.fail_map.clone(),
            self.clr_cfg.clone(),
            self.hbr_cfg.clone(),
          ))
          .await;
      }
      Done => {
        self.finish = true && self.finite;
        self.complete(ctx);
      }
      Kill(port) => {
        let socket = Socket::new(Host::DNS("127.0.0.1".to_string()), port, 0);
        let (mut proc, msgs) = self.nodes.remove(&socket).unwrap();
        proc.kill().unwrap();
        for hdl in msgs.values() {
          hdl.abort();
          self.event_count -= 1;
        }
        for node in self.nodes.keys().cloned().collect_vec() {
          self.expect(ctx, node, ClusterEventSimple::Removed(socket.clone()));
        }
        self.complete(ctx);
      }
      Spawn(port, seeds) => {
        let socket = Socket::new(Host::DNS("127.0.0.1".to_string()), port, 0);
        let mut proc = Command::new("./target/debug/cluster-test-node");
        proc.arg(port.to_string());
        proc.arg("127.0.0.1");
        proc.arg(PORT.to_string());
        for s in seeds.iter() {
          proc.arg(s.to_string());
        }
        for node in self.nodes.keys().cloned().collect_vec() {
          self.expect(ctx, node, ClusterEventSimple::Added(socket.clone()));
        }
        self
          .nodes
          .insert(socket.clone(), (proc.spawn().unwrap(), HashMap::new()));
        let event = if seeds.is_empty()
          || self.nodes.keys().all(|x| !seeds.contains(&x.udp))
        {
          ClusterEventSimple::Alone
        } else {
          ClusterEventSimple::Joined
        };
        self.expect(ctx, socket, event);
      }
      Event(socket, event) => {
        let hdl = self.event(&socket, &event);
        if let Some(hdl) = hdl {
          hdl.abort();
          println!("Received {:?} from {:?}", event, socket);
        } else {
          println!("Unexpected: {:?} from {:?}", event, socket);
          self
            .errors
            .push((MemberErrorType::Unexpected, socket, event));
        }
        self.complete(ctx);
      }
      TimedOut(socket, event) => {
        self.event(&socket, &event).unwrap();
        println!(
          "Timed out: from {:?} expected event {:?}",
          socket.udp, event
        );
        self.errors.push((MemberErrorType::Timeout, socket, event));
        self.complete(ctx);
      }
    }
  }

  async fn post_stop(
    &mut self,
    _: &ActorContext<ClusterNodeTypes, CoordinatorMsg>,
  ) {
    self.notify.send(self.errors.clone()).await.unwrap();
  }
}

fn run_cluster_coordinator_test(
  finite: bool,
  events: Vec<(CoordinatorMsg, Duration)>,
  fail_map: FailureConfigMap,
  clr_cfg: ClusterConfig,
  hbr_cfg: HBRConfig,
) {
  Command::new("cargo").arg("build").status().unwrap();
  let socket = Socket::new(Host::DNS("127.0.0.1".to_string()), PORT, 0);
  let node = Node::<ClusterNodeTypes>::new(socket.clone(), 1).unwrap();
  let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<MemberError>>(1);
  let coor = node.spawn(
    true,
    ClusterCoor {
      finite: finite,
      nodes: HashMap::new(),
      event_count: 0,
      errors: vec![],
      finish: false,
      notify: tx,
      fail_map: fail_map,
      clr_cfg: clr_cfg,
      hbr_cfg: hbr_cfg,
    },
    "coordinator".to_string(),
    true,
  );
  println!("Starting coordinator on port {}", PORT);
  node.rt().spawn(execute_events(coor, events));
  let errors = rx.blocking_recv().unwrap();
  if !errors.is_empty() {
    println!("ERRORS:");
    for (t, s, e) in errors.iter() {
      println!("{:?} from {} for {:?}", t, s.udp, e);
    }
  }
  Command::new("pkill")
    .arg("-f")
    .arg("./target/debug/cluster-test-node")
    .status()
    .unwrap();
  assert!(errors.is_empty());
}

const fn dur(d: u64) -> Duration {
  Duration::from_millis(d)
}

async fn execute_events(
  coor: ActorRef<ClusterNodeTypes, CoordinatorMsg>,
  vec: Vec<(CoordinatorMsg, Duration)>,
) {
  for (msg, dur) in vec.into_iter() {
    sleep(dur).await;
    coor.move_to(msg).await;
  }
}

#[test]
#[allow(dead_code)]
fn cluster_coordinator_test_infinite() {
  let spawn_delay = dur(200);
  let events = vec![
    (Spawn(4000, vec![]), dur(0)),
    (Spawn(4001, vec![4000]), spawn_delay),
    (Spawn(4002, vec![4001]), spawn_delay),
    (Spawn(4003, vec![4002]), spawn_delay),
    (Spawn(4004, vec![4003]), spawn_delay),
    (Spawn(4005, vec![4004]), spawn_delay),
    (Spawn(4006, vec![4005]), spawn_delay),
    (Spawn(4007, vec![4006]), spawn_delay),
    (Spawn(4008, vec![4007]), spawn_delay),
    (Spawn(4009, vec![4008]), spawn_delay),
    (Done, dur(0)),
  ];
  let mut fail = FailureConfigMap::default();
  fail.cluster_wide.drop_prob = 0.5;
  fail.cluster_wide.delay =
    Some((Duration::from_millis(20), Duration::from_millis(50)));
  let mut clr = ClusterConfig::default();
  clr.num_pings = 20;
  let hbr = HBRConfig::default();
  run_cluster_coordinator_test(false, events, fail, clr, hbr);
}

rusty_fork_test! {
  #[test]
  fn cluster_coordinator_test() {
    let spawn_delay = dur(200);
    let kill_delay = dur(500);
    let events = vec![
      (Spawn(4000, vec![]), dur(0)),
      (Spawn(4001, vec![4000]), spawn_delay),
      (Spawn(4002, vec![4001]), spawn_delay),
      (Spawn(4003, vec![4002]), spawn_delay),
      (Spawn(4004, vec![4003]), spawn_delay),
      (Spawn(4005, vec![4004]), spawn_delay),
      (Spawn(4006, vec![4005]), spawn_delay),
      (Spawn(4007, vec![4006]), spawn_delay),
      (Spawn(4008, vec![4007]), spawn_delay),
      (Spawn(4009, vec![4008]), spawn_delay),
      (Kill(4000), kill_delay),
      (Kill(4001), kill_delay),
      (Kill(4002), kill_delay),
      (Kill(4003), kill_delay),
      (Kill(4004), kill_delay),
      (Kill(4005), kill_delay),
      (Kill(4006), kill_delay),
      (Kill(4007), kill_delay),
      (Kill(4008), kill_delay),
      (Kill(4009), kill_delay),
      (Done, dur(0)),
    ];
    let fail = FailureConfigMap::default();
    let clr = ClusterConfig::default();
    let hbr = HBRConfig::default();
    run_cluster_coordinator_test(true, events, fail, clr, hbr);
  }
}
