#![allow(unused_imports, dead_code, unused_variables)]
use async_trait::async_trait;
use aurum::cluster::{Cluster, ClusterEvent};
use aurum::core::{forge, Actor, ActorContext, ActorRef, Host, Node, Socket};
use aurum::test_commons::{ClusterNodeTypes, CoordinatorMsg};
use aurum::{unify, AurumInterface};
use im;
use itertools::Itertools;
use std::collections::HashMap;
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
        let socket = Socket::new(Host::DNS("127.0.0.1".to_string()), port, 0);
        let mut proc = Command::new("cargo");
        proc.stdout(std::process::Stdio::null());
        proc.arg("run");
        proc.arg("--bin");
        proc.arg("cluster-test-node");
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
        }
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
      }
    }
  }
}

fn main() {
  let socket = Socket::new(Host::DNS("127.0.0.1".to_string()), PORT, 0);
  let node = Node::<ClusterNodeTypes>::new(socket.clone(), 1).unwrap();
  let nodes = vec![
    (Spawn(4000, vec![]), dur(500)),
    (Spawn(4001, vec![4000]), dur(500)),
    (Spawn(4002, vec![4001]), dur(500)),
    (Spawn(4003, vec![4002]), dur(500)),
    (Spawn(4004, vec![4003]), dur(500)),
    (Spawn(4005, vec![4004]), dur(500)),
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
