#![allow(unused_imports, dead_code, unused_variables)]

use crate::cluster::{
  Gossip, HBRConfig, HBRState, HeartbeatReceiver, HeartbeatReceiverMsg,
  IntervalStorage, MachineState, NodeRing,
};
use crate::core::{
  udp_msg, Actor, ActorContext, ActorRef, ActorSignal, Case, Destination, Host,
  LocalRef, Node, Socket,
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
  + Case<HeartbeatReceiverMsg>
{
}
impl<T> UnifiedBounds for T where
  T: crate::core::UnifiedBounds
    + Case<ClusterMsg<Self>>
    + Case<IntraClusterMsg<Self>>
    + Case<HeartbeatReceiverMsg>
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

const RELIABLE: bool = true;

pub struct ClusterConfig {
  pub gossip_disperse: usize,
  pub ping_timeout: Duration,
  pub num_pings: usize,
  pub heartbeat_interval: Duration,
  pub seed_nodes: Vec<Socket>,
  pub replication_factor: usize,
}
impl Default for ClusterConfig {
  fn default() -> Self {
    ClusterConfig {
      gossip_disperse: 5,
      ping_timeout: Duration::from_millis(5000),
      num_pings: 1,
      heartbeat_interval: Duration::from_millis(50),
      seed_nodes: vec![],
      replication_factor: 3,
    }
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
  GossipRound,
}

#[derive(Serialize, Deserialize)]
#[serde(bound = "U: UnifiedBounds")]
pub enum IntraClusterMsg<U: UnifiedBounds> {
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
pub enum ClusterEventSimple {
  Alone,
  Joined,
  Added(Socket),
  Removed(Socket),
  Left,
}
impl From<ClusterEvent> for ClusterEventSimple {
  fn from(e: ClusterEvent) -> Self {
    match e {
      ClusterEvent::Added(m) => Self::Added(m.socket.clone()),
      ClusterEvent::Removed(m) => Self::Removed(m.socket.clone()),
      ClusterEvent::Alone(_) => Self::Alone,
      ClusterEvent::Joined(_) => Self::Joined,
      ClusterEvent::Left => Self::Left,
    }
  }
}

#[derive(
  AurumInterface, Clone, Serialize, Deserialize, Hash, PartialEq, Eq, Debug,
)]
pub enum ClusterEvent {
  Alone(Arc<Member>),
  Joined(Arc<Member>),
  Added(Arc<Member>),
  Removed(Arc<Member>),
  Left,
}

struct InCluster {
  charges: HashMap<Arc<Member>, LocalRef<HeartbeatReceiverMsg>>,
  gossip: Gossip,
  ring: NodeRing,
}
impl InCluster {
  fn alone<U: UnifiedBounds>(common: &NodeState<U>) -> InCluster {
    let mut ring = NodeRing::new(common.clr_config.replication_factor);
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
      ReqHeartbeat(_) => {}
      State(gossip) => {
        let events = self.gossip.merge(gossip);
        for e in events {
          match &e {
            ClusterEvent::Added(member) => {
              self.ring.insert(member.clone());
              if self.ring.is_manager(&common.member, member) {
                let hbr = HeartbeatReceiver::spawn(ctx, common, member.clone());
                self.charges.insert(member.clone(), hbr);
              }
            }
            ClusterEvent::Removed(member) => {
              self.ring.remove(&*member).unwrap();
              if self.ring.is_manager(&common.member, member) {
                self
                  .charges
                  .remove(member)
                  .unwrap()
                  .signal(ActorSignal::Term);
              }
            }
            _ => {}
          }
          common.notify(e);
        }
      }
      Ping(member) => {
        println!(
          "{}: received ping from {:?}",
          common.member.socket.udp, member
        );
        self.gossip.states.insert(member.clone(), Up);
        common.gossip_round(&self.gossip).await;
        common.notify(ClusterEvent::Added(member));
      }
    }
    None
  }
}

struct Pinging {
  count: usize,
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
    for s in common.clr_config.seed_nodes.iter() {
      udp_send!(RELIABLE, s, &common.dest, &msg);
    }
    let ar = ctx.local_interface();
    self.timeout = ctx.node.schedule(common.clr_config.ping_timeout, move || {
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
        common.notify(ClusterEvent::Joined(common.member.clone()));
        let mut ring = NodeRing::new(common.clr_config.replication_factor);
        gossip
          .states
          .iter()
          .filter(|(_, s)| s < &&Down)
          .map(|(m, _)| m.clone())
          .for_each(|member| ring.insert(member));
        let charges = ring
          .charges(&common.member)
          .into_iter()
          .map(|m| (m.clone(), HeartbeatReceiver::spawn(ctx, common, m)))
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

pub(crate) struct NodeState<U: UnifiedBounds> {
  pub member: Arc<Member>,
  pub dest: Destination<U>,
  pub subscribers: Vec<LocalRef<ClusterEvent>>,
  pub clr_config: ClusterConfig,
  pub hbr_config: HBRConfig,
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
      .choose_multiple(&mut rand::thread_rng(), self.clr_config.gossip_disperse)
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
    if self.common.clr_config.seed_nodes.is_empty() {
      self.create_cluster();
    } else {
      let mut png = Pinging {
        count: self.common.clr_config.num_pings,
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
    vnodes: u32,
    subrs: Vec<LocalRef<ClusterEvent>>,
    clr_config: ClusterConfig,
    hbr_config: HBRConfig
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
        subscribers: subrs,
        clr_config: clr_config,
        hbr_config: hbr_config,
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
    self
      .common
      .notify(ClusterEvent::Alone(self.common.member.clone()));
    self.state = InteractionState::InCluster(InCluster::alone(&self.common));
  }
}
