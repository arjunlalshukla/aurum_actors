#![allow(unused_imports, dead_code, unused_variables)]
use async_trait::async_trait;
use aurum::cluster::{Cluster, ClusterEvent};
use aurum::core::{forge, Actor, ActorContext, ActorRef, Host, Node, Socket};
use aurum::test_commons::{ClusterNodeTypes, CoordinatorMsg};
use aurum::{cluster::ClusterCmd, core::LocalRef, unify, AurumInterface};
use im;
use std::env::args;

struct ClusterNode {
  members: im::HashSet<Socket>,
  coor: ActorRef<ClusterNodeTypes, CoordinatorMsg>,
  seeds: Vec<Socket>,
}
#[async_trait]
impl Actor<ClusterNodeTypes, ClusterEvent> for ClusterNode {
  async fn pre_start(
    &mut self,
    ctx: &ActorContext<ClusterNodeTypes, ClusterEvent>,
  ) {
    Cluster::new(&ctx.node, "test".to_string(), self.seeds.clone(), 10.0, 3)
      .await
      .send(ClusterCmd::Subscribe(ctx.local_interface()));
  }

  async fn recv(
    &mut self,
    ctx: &ActorContext<ClusterNodeTypes, ClusterEvent>,
    msg: ClusterEvent,
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
  println!("Starting cluster node test on port {}", port);
  println!("Contacting coor on {:?}", coor);
  for seed in seeds.iter() {
    println!("Using seed: {:?}", seed);
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
}
