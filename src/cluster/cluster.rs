#![allow(unused_imports, dead_code, unused_variables)]

use crate::cluster::{Gossip, IntervalStorage, MachineState, NodeRing};
use crate::core::{
  udp_msg, Actor, ActorContext, ActorRef, Case, Destination, Host, LocalRef,
  Node, Socket,
};
use crate::{self as aurum, udp_send};
use async_trait::async_trait;
use aurum_macros::AurumInterface;
use maplit::btreemap;
use rand::seq::IteratorRandom;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
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

const INTERVAL_CAP: usize = 10;
const INTERVAL_INIT: Duration = Duration::from_millis(50);
const INTERVAL_TIMES: usize = 2;
const GOSSIP_DISPERSE: usize = 5;
const RELIABLE: bool = true;
const PING_DELAY: Duration = Duration::from_millis(5000);
const NUM_PINGS: u32 = 1;

#[derive(AurumInterface)]
#[aurum(local)]
pub enum ClusterMsg<U: UnifiedBounds> {
  #[aurum]
  IntraMsg(IntraClusterMsg<U>),
  #[aurum(local)]
  LocalCmd(ClusterCmd),
  PingTimeout,
  GossipRound,
}

#[derive(Serialize, Deserialize)]
#[serde(bound = "U: UnifiedBounds")]
pub enum IntraClusterMsg<U: UnifiedBounds> {
  Heartbeat(Member),
  ReqHeartbeat(ActorRef<U, IntraClusterMsg<U>>),
  State(Gossip),
  Ping(Arc<Member>),
}

pub enum ClusterCmd {
  //Leave,
  //Join,
  //SetHeartbeatInterval(Duration),
  Subscribe(LocalRef<ClusterEvent>),
}

#[derive(
  AurumInterface, Clone, Serialize, Deserialize, Hash, PartialEq, Eq, Debug,
)]
pub enum ClusterEvent {
  Alone,
  Joined,
  Added(Socket),
  Removed(Socket),
  Left,
}

struct InCluster {
  charges: HashMap<Arc<Member>, IntervalStorage>,
  gossip: Gossip,
  ring: NodeRing,
}
impl InCluster {
  fn alone<U: UnifiedBounds>(common: &NodeState<U>) -> InCluster {
    let mut ring = NodeRing::new(common.rep_factor);
    ring.insert(common.member.clone());
    InCluster {
      charges: HashMap::new(),
      gossip: Gossip {
        states: btreemap! {common.member.clone() => Up},
      },
      ring: ring,
    }
  }

  async fn process<U: UnifiedBounds>(
    &mut self,
    common: &mut NodeState<U>,
    ctx: &ActorContext<U, ClusterMsg<U>>,
    msg: IntraClusterMsg<U>,
  ) -> Option<InteractionState> {
    match msg {
      Heartbeat(_) | ReqHeartbeat(_) => {}
      State(gossip) => {
        let events = self.gossip.merge(gossip);
        for e in events {
          common.notify(e);
        }
      }
      Ping(member) => {
        println!(
          "{}: received ping from {:?}",
          common.member.socket.udp, member
        );
        let socket = member.socket.clone();
        self.gossip.states.insert(member, Up);
        //let msg: IntraClusterMsg<U> = State(self.gossip.clone());
        //udp_send!(RELIABLE, &common.member.socket, &common.dest, &msg);
        common.gossip_round(&self.gossip).await;
        common.notify(ClusterEvent::Added(socket));
      }
    }
    None
  }
}

struct Pinging {
  count: u32,
  timeout: JoinHandle<()>,
}
impl Pinging {
  async fn ping<U: UnifiedBounds>(
    &mut self,
    common: &NodeState<U>,
    ctx: &ActorContext<U, ClusterMsg<U>>,
  ) {
    self.count -= 1;
    println!(
      "{}: starting ping. {} attempts left",
      common.member.socket.udp, self.count
    );
    let msg: IntraClusterMsg<U> = Ping(common.member.clone());
    for s in common.seeds.iter() {
      udp_send!(RELIABLE, s, &common.dest, &msg);
    }
    let ar = ctx.local_interface();
    self.timeout = ctx.node.schedule(PING_DELAY, move || {
      ar.send(ClusterMsg::PingTimeout);
    });
  }

  async fn process<U: UnifiedBounds>(
    &self,
    common: &mut NodeState<U>,
    ctx: &ActorContext<U, ClusterMsg<U>>,
    msg: IntraClusterMsg<U>,
  ) -> Option<InteractionState> {
    match msg {
      State(gossip) => {
        common.gossip_round(&gossip).await;
        common.notify(ClusterEvent::Joined);
        let mut ring = NodeRing::new(common.rep_factor);
        gossip
          .states
          .iter()
          .filter(|(_, s)| s < &&Down)
          .map(|(m, _)| m.clone())
          .for_each(|member| ring.insert(member));
        let charges = ring
          .charges(&common.member)
          .into_iter()
          .map(|mem| {
            (
              mem,
              IntervalStorage::new(
                INTERVAL_CAP,
                INTERVAL_INIT,
                INTERVAL_TIMES,
                None,
              ),
            )
          })
          .collect();
        Some(InteractionState::InCluster(InCluster {
          charges: charges,
          gossip: gossip,
          ring: ring,
        }))
      }
      _ => None,
    }
  }
}

enum InteractionState {
  InCluster(InCluster),
  Pinging(Pinging),
  Left,
}
impl InteractionState {
  async fn process<U: UnifiedBounds>(
    &mut self,
    common: &mut NodeState<U>,
    ctx: &ActorContext<U, ClusterMsg<U>>,
    msg: IntraClusterMsg<U>,
  ) {
    let new_state = match self {
      InteractionState::InCluster(ref mut state) => {
        state.process(common, ctx, msg).await
      }
      InteractionState::Pinging(ref mut state) => {
        state.process(common, ctx, msg).await
      }
      InteractionState::Left => None,
    };
    if let Some(s) = new_state {
      *self = s;
    }
  }
}

struct NodeState<U: UnifiedBounds> {
  member: Arc<Member>,
  dest: Destination<U>,
  seeds: Vec<Socket>,
  subscribers: Vec<LocalRef<ClusterEvent>>,
  rep_factor: usize,
  ping_attempts: u32,
  phi: f64,
}
impl<U: UnifiedBounds> NodeState<U> {
  fn notify(&mut self, event: ClusterEvent) {
    self.subscribers.retain(|s| s.send(event.clone()));
  }

  async fn gossip_round(&self, gossip: &Gossip) {
    let msg: IntraClusterMsg<U> = State(gossip.clone());
    let members = gossip
      .states
      .iter()
      .filter(|(m, s)| (**m) != self.member && s < &&Down)
      .map(|(m, _)| m)
      .choose_multiple(&mut rand::thread_rng(), GOSSIP_DISPERSE)
      .into_iter();
    for member in members {
      udp_send!(RELIABLE, &member.socket, &self.dest, &msg)
    }
  }
}

pub struct Cluster<U: UnifiedBounds> {
  common: NodeState<U>,
  state: InteractionState,
}
#[async_trait]
impl<U: UnifiedBounds> Actor<U, ClusterMsg<U>> for Cluster<U> {
  async fn pre_start(&mut self, ctx: &ActorContext<U, ClusterMsg<U>>) {
    if self.common.seeds.is_empty() {
      self.create_cluster();
    } else {
      let mut png = Pinging {
        count: self.common.ping_attempts,
        timeout: ctx.node.rt().spawn(async {}),
      };
      png.ping(&self.common, ctx).await;
      self.state = InteractionState::Pinging(png);
    }
  }

  async fn recv(
    &mut self,
    ctx: &ActorContext<U, ClusterMsg<U>>,
    msg: ClusterMsg<U>,
  ) {
    match msg {
      ClusterMsg::IntraMsg(msg) => {
        self.state.process(&mut self.common, ctx, msg).await;
      }
      ClusterMsg::LocalCmd(ClusterCmd::Subscribe(subr)) => {
        self.common.subscribers.push(subr);
      }
      ClusterMsg::PingTimeout => {
        if let InteractionState::Pinging(png) = &mut self.state {
          if png.count != 0 {
            png.ping(&self.common, ctx).await;
          } else {
            self.create_cluster();
          }
        }
      }
      ClusterMsg::GossipRound => {
        if let InteractionState::InCluster(ic) = &self.state {
          self.common.gossip_round(&ic.gossip).await;
        }
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
    vnodes: u32,
    subrs: Vec<LocalRef<ClusterEvent>>,
  ) -> LocalRef<ClusterCmd> {
    let c = Cluster {
      common: NodeState {
        member: Arc::new(Member {
          socket: node.socket().clone(),
          id: rand::random(),
          vnodes: vnodes,
        }),
        dest: Destination::new::<ClusterMsg<U>, IntraClusterMsg<U>>(
          name.clone(),
        ),
        seeds: seeds,
        subscribers: subrs,
        rep_factor: 3,
        ping_attempts: NUM_PINGS,
        phi: phi,
      },
      state: InteractionState::Left,
    };
    node
      .spawn(false, c, name, true)
      .local()
      .clone()
      .unwrap()
      .transform()
  }

  fn create_cluster(&mut self) {
    self.common.notify(ClusterEvent::Alone);
    self.state = InteractionState::InCluster(InCluster::alone(&self.common));
  }
}
