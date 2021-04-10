#![allow(unused_imports, dead_code, unused_variables)]
use async_trait::async_trait;
use aurum::cluster::{Cluster, ClusterEvent};
use aurum::core::{forge, Actor, ActorContext, ActorRef, ActorSignal, Host, Node, Socket};
use aurum::test_commons::{ClusterNodeTypes, CoordinatorMsg};
use aurum::{unify, AurumInterface};
use im;
use itertools::Itertools;
use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use CoordinatorMsg::*;

const PORT: u16 = 3000;
const TIMEOUT: Duration = Duration::from_millis(10_000);

#[derive(Clone, Debug)]
enum MemberErrorType {
  Timeout,
  Unexpected
}

type MemberError = (MemberErrorType, Socket, ClusterEvent);

struct ClusterCoor {
  nodes: HashMap<Socket, (Child, HashMap<ClusterEvent, JoinHandle<bool>>)>,
  errors: Vec<MemberError>,
  finish: bool,
  notify: tokio::sync::mpsc::Sender<Vec<MemberError>>
}
impl ClusterCoor {
  fn expect(
    &mut self,
    ctx: &ActorContext<ClusterNodeTypes, CoordinatorMsg>,
    node: Socket,
    event: ClusterEvent,
  ) {
    let events = &mut self.nodes.get_mut(&node).unwrap().1;
    let timeout = ctx.node.schedule_local_msg(
      TIMEOUT,
      ctx.local_interface(),
      CoordinatorMsg::TimedOut(node, event.clone()),
    );
    events.insert(event, timeout);
  }

  fn is_empty(&self) -> bool {
    self.nodes.is_empty() || self.nodes.values().all(|(_, es)| es.is_empty())
  }

  fn complete(&self, ctx: &ActorContext<ClusterNodeTypes, CoordinatorMsg>) {
    if self.finish && self.is_empty() {
      ctx.local_interface().signal(ActorSignal::Term);
    }
  }
}
#[async_trait]
impl Actor<ClusterNodeTypes, CoordinatorMsg> for ClusterCoor {
  async fn recv(
    &mut self,
    ctx: &aurum::core::ActorContext<ClusterNodeTypes, CoordinatorMsg>,
    msg: CoordinatorMsg,
  ) {
    match msg {
      Done => {
        self.finish = true;
        self.complete(ctx);
      }
      Kill(port) => {
        let socket = Socket::new(Host::DNS("127.0.0.1".to_string()), port, 0);
        let (mut proc, msgs) = self.nodes.remove(&socket).unwrap();
        proc.kill().unwrap();
        msgs.values().for_each(|to| to.abort());
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
          self.expect(ctx, node, ClusterEvent::Added(socket.clone()));
        }
        self
          .nodes
          .insert(socket.clone(), (proc.spawn().unwrap(), HashMap::new()));
        let event = if seeds.is_empty()
          || self.nodes.keys().all(|x| !seeds.contains(&x.udp))
        {
          ClusterEvent::Alone
        } else {
          ClusterEvent::Joined
        };
        self.expect(ctx, socket, event);
      }
      Event(socket, event) => {
        let hdl = self.nodes.get_mut(&socket).unwrap().1.remove(&event);
        if let Some(hdl) = hdl {
          hdl.abort();
          println!("Received {:?} from {:?}", event, socket);
        } else {
          println!("Unexpected: {:?} from {:?}", event, socket);
          self.errors.push((MemberErrorType::Unexpected, socket, event));
        }
        self.complete(ctx);
      }
      TimedOut(socket, event) => {
        self
          .nodes
          .get_mut(&socket)
          .unwrap()
          .1
          .remove(&event)
          .unwrap();
        println!(
          "Timed out: from {:?} expected event {:?}",
          socket.udp, event
        );
        self.errors.push((MemberErrorType::Timeout, socket, event));
        self.complete(ctx);
      }
    }
  }

  async fn post_stop(&mut self, ctx: &ActorContext<ClusterNodeTypes, CoordinatorMsg>) {

    self.notify.send(self.errors.clone()).await.unwrap();
  }
}

#[test]
fn cluster_coordinator_test() {
  Command::new("cargo").arg("build").spawn().unwrap().wait().unwrap();
  let socket = Socket::new(Host::DNS("127.0.0.1".to_string()), PORT, 0);
  let node = Node::<ClusterNodeTypes>::new(socket.clone(), 1).unwrap();
  let millis = 500;
  let nodes = vec![
    (Spawn(4000, vec![]), dur(0)),
    (Spawn(4001, vec![4000]), dur(millis)),
    (Spawn(4002, vec![4001]), dur(millis)),
    (Spawn(4003, vec![4002]), dur(millis)),
    (Spawn(4004, vec![4003]), dur(millis)),
    (Spawn(4005, vec![4004]), dur(millis)),
    (Done, dur(0)),
  ];
  let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<MemberError>>(1);
  let coor = node.spawn(
    true,
    ClusterCoor {
      nodes: HashMap::new(),
      errors: vec![],
      finish: false,
      notify: tx
    },
    "coordinator".to_string(),
    true,
  );
  println!("Starting coordinator on port {}", PORT);
  node.rt().spawn(events(coor, nodes));
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
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(errors.is_empty());
}

const fn dur(d: u64) -> Duration {
  Duration::from_millis(d)
}

async fn events(
  coor: ActorRef<ClusterNodeTypes, CoordinatorMsg>,
  vec: Vec<(CoordinatorMsg, Duration)>,
) {
  for (msg, dur) in vec.into_iter() {
    sleep(dur).await;
    coor.move_to(msg).await;
  }
}