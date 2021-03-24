#![allow(unused_imports, dead_code, unused_variables)]

use crate as aurum;
use crate::cluster::IntervalStorage;
use crate::core::{
  forge, udp_msg, Actor, ActorContext, ActorRef, Case, Destination, LocalRef,
  Node, Socket,
};
use async_trait::async_trait;
use aurum_macros::AurumInterface;
use im;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use tokio::task::JoinHandle;

pub trait UnifiedBounds:
  crate::core::UnifiedBounds
  + Case<ClusterMsg<Self>>
  + Case<IntraClusterMsg<Self>>
{
}
impl<T> UnifiedBounds for T where
  T: crate::core::UnifiedBounds
    + Case<ClusterMsg<Self>>
    + Case<IntraClusterMsg<Self>>
{
}

#[derive(AurumInterface)]
#[aurum(local)]
pub enum ClusterMsg<U: UnifiedBounds> {
  #[aurum]
  IntraMsg(IntraClusterMsg<U>),
  #[aurum(local)]
  LocalCmd(ClusterCmd),
}

#[derive(Serialize, Deserialize)]
#[serde(bound = "U: UnifiedBounds")]
pub enum IntraClusterMsg<U: UnifiedBounds> {
  Heartbeat(ActorRef<U, IntraClusterMsg<U>>),
  ReqHeartbeat(ActorRef<U, IntraClusterMsg<U>>),
}

pub enum ClusterCmd {
  //Leave,
  Subscribe(LocalRef<ClusterEvent>),
}

pub enum ClusterEventType {}

#[derive(
  AurumInterface, Clone, Serialize, Deserialize, Hash, PartialEq, Eq, Debug,
)]
pub enum ClusterEvent {
  Members(im::HashSet<Socket>),
  Left(Socket),
  Downed(Socket),
  Added(Socket),
}

struct InCluster {
  trackees: HashMap<Socket, IntervalStorage>,
  members: im::HashSet<Socket>,
  ring: BTreeMap<u64, Socket>
}

// Looking through the consistent hashing ring, log(n) time.
// ring.range(key..).chain(ring.range(..key)).unique().take(n)

struct Pinging {
  count: u32,
  timeout: JoinHandle<()>,
}

enum NodePrivateState {
  InCluster(InCluster),
  Pinging(Pinging),
}

struct ClusterState<U: UnifiedBounds> {
  dest: Destination<U>,
  seeds: Vec<Socket>,
  subscribers: Vec<LocalRef<ClusterEvent>>,
  max_tries: u32,
  phi: f64,
}

pub struct Cluster<U: UnifiedBounds> {
  common: ClusterState<U>,
  state: NodePrivateState,
}

#[async_trait]
impl<U: UnifiedBounds> Actor<U, ClusterMsg<U>> for Cluster<U> {
  async fn recv(
    &mut self,
    ctx: &ActorContext<U, ClusterMsg<U>>,
    msg: ClusterMsg<U>,
  ) {
    match msg {
      ClusterMsg::IntraMsg(msg) => {
        let new_state = match &mut self.state {
          NodePrivateState::InCluster(ref mut state) => {
            Self::in_cluster(&mut self.common, state, ctx, msg).await
          }
          NodePrivateState::Pinging(ref mut state) => {
            Self::pinging(&mut self.common, state, ctx, msg).await
          }
        };
        if let Some(s) = new_state {
          self.state = s;
        }
      }
      ClusterMsg::LocalCmd(ClusterCmd::Subscribe(subr)) => {
        self.common.subscribers.push(subr);
      }
    }
  }
}
impl<U: UnifiedBounds> Cluster<U> {
  pub async fn new(
    node: &Node<U>,
    name: String,
    seeds: Vec<Socket>,
    phi: f64,
  ) -> LocalRef<ClusterCmd> {
    let c = Cluster {
      common: ClusterState {
        dest: Destination::new::<ClusterMsg<U>, IntraClusterMsg<U>>(
          name.clone(),
        ),
        seeds: seeds,
        subscribers: vec![],
        max_tries: 0,
        phi: phi,
      },
      state: NodePrivateState::Pinging(Pinging {
        count: 0,
        timeout: node.rt().spawn(async {}),
      }),
    };
    node
      .spawn(false, c, name, true)
      .local()
      .clone()
      .unwrap()
      .transform()
  }

  pub async fn start(&self, ctx: &ActorContext<U, ClusterMsg<U>>) {
    let msg = IntraClusterMsg::Heartbeat(ctx.interface());
    for s in self.common.seeds.iter() {
      udp_msg(s, &self.common.dest, &msg).await;
    }
  }

  async fn pinging(
    common: &mut ClusterState<U>,
    state: &mut Pinging,
    ctx: &ActorContext<U, ClusterMsg<U>>,
    msg: IntraClusterMsg<U>,
  ) -> Option<NodePrivateState> {
    None
  }

  async fn in_cluster(
    common: &mut ClusterState<U>,
    state: &mut InCluster,
    ctx: &ActorContext<U, ClusterMsg<U>>,
    msg: IntraClusterMsg<U>,
  ) -> Option<NodePrivateState> {
    None
  }
}
