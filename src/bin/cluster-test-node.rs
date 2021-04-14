#![allow(unused_imports, dead_code, unused_variables)]
use async_trait::async_trait;
use aurum::cluster::{Cluster, ClusterConfig, ClusterEventSimple, HBRConfig};
use aurum::core::{forge, Actor, ActorContext, ActorRef, Host, Node, Socket};
use aurum::test_commons::{ClusterNodeTypes, CoordinatorMsg};
use aurum::{cluster::ClusterCmd, core::LocalRef, unify, AurumInterface};
use im;
use std::env::args;
use std::time::Duration;
use tokio::time::sleep;

struct ClusterNode {
  members: im::HashSet<Socket>,
  coor: ActorRef<ClusterNodeTypes, CoordinatorMsg>,
  seeds: Vec<Socket>,
}
#[async_trait]
impl Actor<ClusterNodeTypes, ClusterEventSimple> for ClusterNode {
  async fn pre_start(
    &mut self,
    ctx: &ActorContext<ClusterNodeTypes, ClusterEventSimple>,
  ) {
    let mut config = ClusterConfig::default();
    config.seed_nodes = self.seeds.clone();
    Cluster::new(
      &ctx.node,
      "test".to_string(),
      3,
      vec![ctx.local_interface()],
      config,
      HBRConfig::default(),
    )
    .await;
  }

  async fn recv(
    &mut self,
    ctx: &ActorContext<ClusterNodeTypes, ClusterEventSimple>,
    msg: ClusterEventSimple,
  ) {
    self
      .coor
      .move_to(CoordinatorMsg::Event(ctx.node.socket().clone(), msg))
      .await;
  }
}

fn main() {
  let mut args = args().into_iter();
  args.next();
  let port = args.next().unwrap().parse::<u16>().unwrap();
  let socket = Socket::new(Host::DNS("127.0.0.1".to_string()), port, 0);
  let node = Node::<ClusterNodeTypes>::new(socket.clone(), 1).unwrap();
  let coor = Socket::new(
    Host::DNS(args.next().unwrap().to_string()),
    args.next().unwrap().parse::<u16>().unwrap(),
    0,
  );
  let seeds = args
    .map(|arg| {
      Socket::new(
        Host::DNS("127.0.0.1".to_string()),
        arg.parse::<u16>().unwrap(),
        0,
      )
    })
    .collect::<Vec<_>>();
  println!("STARTED {}, contacting coor on {:?}", socket.udp, coor);
  for seed in seeds.iter() {
    println!("{}: using seed {}", socket.udp, seed.udp);
  }
  node.spawn(
    true,
    ClusterNode {
      members: im::HashSet::new(),
      coor: forge::<_, CoordinatorMsg, _>("coordinator".to_string(), coor),
      seeds: seeds,
    },
    format!("node-{}", port),
    true,
  );
  node
    .rt()
    .block_on(async { sleep(Duration::from_secs(0xffffffff)).await });
}
