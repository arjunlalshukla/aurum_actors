#![allow(unused_imports, dead_code, unused_variables)]

use crate::cluster::{Gossip, IntervalStorage, MachineState, NodeRing};
use crate::core::{
  forge, udp_msg, Actor, ActorContext, ActorRef, Case, Destination, LocalRef,
  Node, Socket,
};
use crate::{self as aurum, udp_send};
use async_trait::async_trait;
use aurum_macros::AurumInterface;
use crdts::VClock;
use im;
use im::hashmap::Entry;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use tokio::task::JoinHandle;

use IntraClusterMsg::*;
use MachineState::*;

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

#[derive(Serialize, Deserialize, Hash, Eq, Clone, Ord, PartialOrd, Debug)]
pub struct Member {
  pub socket: Socket,
  pub id: u64,
  pub vnodes: u32,
}
impl PartialEq for Member {
  fn eq(&self, other: &Self) -> bool {
    // Should pretty much always take this path. Branch prediction hints?
    if self.id != other.id {
      return false;
    }
    if self.socket != other.socket {
      return false;
    }
    self.vnodes == other.vnodes
  }
}

#[derive(AurumInterface)]
#[aurum(local)]
pub enum ClusterMsg<U: UnifiedBounds> {
  #[aurum]
  IntraMsg(IntraClusterMsg<U>),
  #[aurum(local)]
  LocalCmd(ClusterCmd),
  PingTimeout,
}

#[derive(Serialize, Deserialize)]
#[serde(bound = "U: UnifiedBounds")]
pub enum IntraClusterMsg<U: UnifiedBounds> {
  Heartbeat(Socket),
  ReqHeartbeat(ActorRef<U, IntraClusterMsg<U>>),
  State(Gossip),
  Ping(Socket),
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
  charges: HashMap<Member, IntervalStorage>,
  gossip: Gossip,
  ring: NodeRing,
}

struct Pinging {
  count: u32,
  timeout: JoinHandle<()>,
}

enum InteractionState {
  InCluster(InCluster),
  Pinging(Pinging),
  Left,
}

struct NodeState<U: UnifiedBounds> {
  dest: Destination<U>,
  seeds: Vec<Socket>,
  subscribers: Vec<LocalRef<ClusterEvent>>,
  vnodes: u32,
  ping_attempts: u32,
  phi: f64,
}

pub struct Cluster<U: UnifiedBounds> {
  common: NodeState<U>,
  state: InteractionState,
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
          InteractionState::InCluster(ref mut state) => {
            Self::in_cluster(&mut self.common, state, ctx, msg).await
          }
          InteractionState::Pinging(ref mut state) => {
            Self::pinging(&mut self.common, state, ctx, msg).await
          }
          InteractionState::Left => None,
        };
        if let Some(s) = new_state {
          self.state = s;
        }
      }
      ClusterMsg::LocalCmd(ClusterCmd::Subscribe(subr)) => {
        self.common.subscribers.push(subr);
      }
      ClusterMsg::PingTimeout => {}
    }
  }
}
impl<U: UnifiedBounds> Cluster<U> {
  pub async fn new(
    node: &Node<U>,
    name: String,
    seeds: Vec<Socket>,
    phi: f64,
    vnodes: u32,
  ) -> LocalRef<ClusterCmd> {
    let c = Cluster {
      common: NodeState {
        dest: Destination::new::<ClusterMsg<U>, IntraClusterMsg<U>>(
          name.clone(),
        ),
        seeds: seeds,
        subscribers: vec![],
        vnodes,
        ping_attempts: 5,
        phi: phi,
      },
      state: InteractionState::Pinging(Pinging {
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

  async fn start(&self, ctx: &ActorContext<U, ClusterMsg<U>>) {
    let msg: IntraClusterMsg<U> = Ping(ctx.node.socket().clone());
    for s in self.common.seeds.iter() {
      udp_send!(false, s, &self.common.dest, &msg);
    }
  }

  async fn pinging(
    common: &mut NodeState<U>,
    state: &mut Pinging,
    ctx: &ActorContext<U, ClusterMsg<U>>,
    msg: IntraClusterMsg<U>,
  ) -> Option<InteractionState> {
    match msg {
      State(s) => None,
      _ => None,
    }
  }

  async fn in_cluster(
    common: &mut NodeState<U>,
    state: &mut InCluster,
    ctx: &ActorContext<U, ClusterMsg<U>>,
    msg: IntraClusterMsg<U>,
  ) -> Option<InteractionState> {
    None
  }
}
