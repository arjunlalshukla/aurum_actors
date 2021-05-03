use async_trait::async_trait;
use aurum::cluster::{Cluster, ClusterCmd};
use aurum::core::{
  forge, Actor, ActorContext, ActorRef, Host, LocalRef, Node, Socket,
};
use aurum::test_commons::{ClusterNodeMsg, ClusterNodeTypes, CoordinatorMsg};
use aurum::testkit::FailureConfigMap;
use std::env::args;
use std::time::Duration;
use tokio::time::sleep;
use ClusterNodeState::*;

enum ClusterNodeState {
  Initial,
  InCluster(FailureConfigMap, LocalRef<ClusterCmd>),
}

struct ClusterNode {
  state: ClusterNodeState,
  coor: ActorRef<ClusterNodeTypes, CoordinatorMsg>,
  seeds: Vec<Socket>,
}
#[async_trait]
impl Actor<ClusterNodeTypes, ClusterNodeMsg> for ClusterNode {
  async fn pre_start(
    &mut self,
    ctx: &ActorContext<ClusterNodeTypes, ClusterNodeMsg>,
  ) {
    self.coor.move_to(CoordinatorMsg::Up(ctx.interface())).await;
  }

  async fn recv(
    &mut self,
    ctx: &ActorContext<ClusterNodeTypes, ClusterNodeMsg>,
    msg: ClusterNodeMsg,
  ) {
    match &mut self.state {
      Initial => match msg {
        ClusterNodeMsg::FailureMap(map, mut clr, hbr) => {
          clr.seed_nodes = self.seeds.clone();
          let cluster = Cluster::new(
            &ctx.node,
            "test".to_string(),
            3,
            vec![ctx.local_interface()],
            map.clone(),
            clr,
            hbr,
          )
          .await;
          self.state = InCluster(map, cluster);
        }
        _ => unreachable!(),
      },
      InCluster(ref mut fail_map, cluster) => match msg {
        ClusterNodeMsg::Update(update) => {
          self
            .coor
            .move_to(CoordinatorMsg::Event(
              ctx.node.socket().clone(),
              update.event.into(),
            ))
            .await;
        }
        ClusterNodeMsg::FailureMap(map, _, _) => {
          *fail_map = map.clone();
          cluster.send(ClusterCmd::FailureMap(map));
        }
      },
    }
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
  node.spawn(
    true,
    ClusterNode {
      state: Initial,
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
