#![allow(unused_imports, dead_code, unused_variables)]
use async_trait::async_trait;
use aurum::cluster::{Cluster, ClusterEvent};
use aurum::core::{forge, Actor, ActorRef, Host, Node, Socket};
use aurum::test_commons::{ClusterNodeTypes, CoordinatorMsg};
use aurum::{unify, AurumInterface};
use im;
use std::collections::HashMap;
use std::env::args;
use std::process::{Child, Command};
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use CoordinatorMsg::*;

const PORT: u16 = 3000;
const TIMEOUT: Duration = Duration::from_millis(60_000);

struct ClusterCoor {
  nodes: HashMap<Socket, (Child, HashMap<ClusterEvent, JoinHandle<()>>)>,
}
#[async_trait]
impl Actor<ClusterNodeTypes, CoordinatorMsg> for ClusterCoor {
  async fn recv(
    &mut self,
    ctx: &aurum::core::ActorContext<ClusterNodeTypes, CoordinatorMsg>,
    msg: CoordinatorMsg,
  ) {
    match msg {
      Kill(port) => {
        let socket = Socket::new(Host::DNS("127.0.0.1".to_string()), port, 0);
        let (mut proc, msgs) = self.nodes.remove(&socket).unwrap();
        proc.kill().unwrap();
        msgs.values().for_each(|to| to.abort());
      }
      Spawn(port, seeds) => {
        let mut proc = Command::new("cargo");
        proc.arg("run");
        proc.arg("--bin");
        proc.arg("cluster-test-node");
        proc.arg(port.to_string());
        proc.arg("127.0.0.1");
        proc.arg(PORT.to_string());
        for s in seeds.iter() {
          proc.arg(s.to_string());
        }
        let proc = proc.spawn().unwrap();
        let msgs = HashMap::new();
        let socket = Socket::new(Host::DNS("127.0.0.1".to_string()), port, 0);
        for (node, (_, events)) in self.nodes.iter_mut() {
          let event = ClusterEvent::Added(socket.clone());
          let ec = event.clone();
          let sc = socket.clone();
          let actor = ctx.local_interface();
          let node = node.clone();
          let timeout = ctx.node.rt().spawn(async move {
            sleep(TIMEOUT).await;
            actor.send(CoordinatorMsg::TimedOut(node, ec));
          });
          events.insert(event, timeout);
        }
        self.nodes.insert(socket, (proc, msgs));
      }
      Event(socket, event) => {
        self
          .nodes
          .get_mut(&socket)
          .unwrap()
          .1
          .remove(&event)
          .unwrap()
          .abort();
        println!("Received {:?} from {:?}", event, socket);
      }
      TimedOut(socket, event) => {
        self
          .nodes
          .get_mut(&socket)
          .unwrap()
          .1
          .remove(&event)
          .unwrap();
        println!("Expected event {:?} from {:?} timed out", event, socket);
      }
    }
  }
}

fn main() {
  let socket = Socket::new(Host::DNS("127.0.0.1".to_string()), PORT, 0);
  let node = Node::<ClusterNodeTypes>::new(socket.clone(), 1).unwrap();
  let nodes = vec![
    (Spawn(4000, vec![]), dur(5_000)),
    (Spawn(4001, vec![4000]), dur(5_000)),
    (Spawn(4002, vec![4001]), dur(5_000)),
    (Spawn(4003, vec![4002]), dur(5_000)),
    (Spawn(4004, vec![4003]), dur(5_000)),
    (Spawn(4005, vec![4004]), dur(5_000)),
  ];
  let coor = node.spawn(
    true,
    ClusterCoor {
      nodes: HashMap::new(),
    },
    "coordinator".to_string(),
    true,
  );
  node.rt().spawn(events(coor, nodes));
  println!("Started coordinator on port {}", PORT);
  node
    .rt()
    .block_on(async { sleep(Duration::from_secs(0xffffffff)).await });
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
