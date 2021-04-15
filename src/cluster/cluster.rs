//#![allow(unused_imports, dead_code, unused_variables)]

use crate as aurum;
use crate::cluster::{
  Gossip, HBRConfig, HeartbeatReceiver, HeartbeatReceiverMsg, MachineState,
  NodeRing, RELIABLE,
};
use crate::core::{
  Actor, ActorContext, ActorRef, ActorSignal, Case, Destination, LocalRef,
  Node, Socket,
};
use crate::{udp_send, AurumInterface};
use async_trait::async_trait;
use itertools::Itertools;
use maplit::btreemap;
use rand::seq::IteratorRandom;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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

pub struct ClusterConfig {
  pub gossip_timeout: Duration,
  pub gossip_disperse: usize,
  pub ping_timeout: Duration,
  pub num_pings: usize,
  pub hb_interval: Duration,
  pub seed_nodes: Vec<Socket>,
  pub replication_factor: usize,
}
impl Default for ClusterConfig {
  fn default() -> Self {
    ClusterConfig {
      gossip_timeout: Duration::from_millis(1000),
      gossip_disperse: 1,
      ping_timeout: Duration::from_millis(500),
      num_pings: 1,
      hb_interval: Duration::from_millis(50),
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
  GossipTimeout,
  Downed(Arc<Member>),
}

#[derive(Serialize, Deserialize)]
#[serde(bound = "U: UnifiedBounds")]
pub enum IntraClusterMsg<U: UnifiedBounds> {
  Foo(ActorRef<U, IntraClusterMsg<U>>),
  ReqHeartbeat(Arc<Member>, u64),
  ReqGossip(Arc<Member>),
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
      Foo(_) => {}
      ReqGossip(member) => {}
      ReqHeartbeat(member, id) => {
        //TODO: What if requester is not the manager?
        if id == common.member.id {
          let msg = HeartbeatReceiverMsg::Heartbeat(
            common.clr_config.hb_interval,
            common.hb_interval_changes,
          );
          udp_send!(RELIABLE, &member.socket, &common.hbr_dest, &msg);
        } else {
          println!(
            "{}: Got HB for id {} when id is {}",
            common.member.socket.udp, common.member.id, id
          );
        }
      }
      State(gossip) => {
        let events = self.gossip.merge(gossip);
        for e in events {
          match &e {
            ClusterEvent::Added(member) => {
              if self.ring.is_manager(&common.member, member) {
                let hbr = HeartbeatReceiver::spawn(ctx, common, member.clone());
                self.charges.insert(member.clone(), hbr);
              }
              self.ring.insert(member.clone());
            }
            ClusterEvent::Removed(member) => {
              if self.ring.is_manager(&common.member, member) {
                self
                  .charges
                  .remove(member)
                  .unwrap()
                  .signal(ActorSignal::Term);
              }
              self.ring.remove(&*member).unwrap();
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
        let mut set = HashSet::new();
        set.insert(&member);
        common.gossip_round(&self.gossip, set).await;
        common.notify(ClusterEvent::Added(member));
      }
    }
    None
  }

  async fn down<U: UnifiedBounds>(
    &mut self,
    common: &mut NodeState<U>,
    member: Arc<Member>,
  ) {
    self
      .charges
      .remove(&member)
      .unwrap()
      .signal(ActorSignal::Term);
    self.ring.remove(&member).unwrap();
    let state = self.gossip.states.get_mut(&member).unwrap();
    *state = Down;
    common.gossip_round(&self.gossip, HashSet::new()).await;
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
      udp_send!(RELIABLE, s, &common.clr_dest, &msg);
    }
    let ar = ctx.local_interface();
    self.timeout =
      ctx.node.schedule(common.clr_config.ping_timeout, move || {
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
        common.gossip_round(&gossip, HashSet::new()).await;
        common.notify(ClusterEvent::Joined(common.member.clone()));
        let mut ring = NodeRing::new(common.clr_config.replication_factor);
        gossip
          .states
          .iter()
          .filter(|(_, s)| s < &&Down)
          .map(|(m, _)| m.clone())
          .for_each(|member| ring.insert(member));
        let charges: HashMap<Arc<Member>, _> = ring
          .charges(&common.member)
          .into_iter()
          .map(|m| (m.clone(), HeartbeatReceiver::spawn(ctx, common, m)))
          .collect();
        println!(
          "{}: responsible for {:?}",
          common.member.socket.udp,
          charges.keys().map(|m| (m.socket.udp, m.id)).collect_vec()
        );
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
  pub clr_dest: Destination<U, IntraClusterMsg<U>>,
  pub hbr_dest: Destination<U, HeartbeatReceiverMsg>,
  pub subscribers: Vec<LocalRef<ClusterEvent>>,
  pub hb_interval_changes: u32,
  pub clr_config: ClusterConfig,
  pub hbr_config: HBRConfig,
}
impl<U: UnifiedBounds> NodeState<U> {
  fn notify(&mut self, event: ClusterEvent) {
    self.subscribers.retain(|s| s.send(event.clone()));
  }

  async fn gossip_round(
    &self,
    gossip: &Gossip,
    guaranteed: HashSet<&Arc<Member>>,
  ) {
    let msg: IntraClusterMsg<U> = State(gossip.clone());
    let members = gossip
      .states
      .iter()
      .filter(|(m, s)| {
        (**m) != self.member && s < &&Down && !guaranteed.contains(*m)
      })
      .map(|(m, _)| m)
      .choose_multiple(&mut rand::thread_rng(), self.clr_config.gossip_disperse)
      .into_iter()
      .chain(guaranteed.into_iter());
    for member in members {
      udp_send!(RELIABLE, &member.socket, &self.clr_dest, &msg)
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
      ClusterMsg::GossipTimeout => {
        if let InteractionState::InCluster(ic) = &self.state {
          self.common.gossip_round(&ic.gossip, HashSet::new()).await;
        }
      }
      ClusterMsg::Downed(member) => {
        if let InteractionState::InCluster(ic) = &mut self.state {
          ic.down(&mut self.common, member).await;
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
    hbr_config: HBRConfig,
  ) -> LocalRef<ClusterCmd> {
    let id = rand::random();
    let c = Cluster {
      common: NodeState {
        member: Arc::new(Member {
          socket: node.socket().clone(),
          id: id,
          vnodes: vnodes,
        }),
        clr_dest: Destination::new::<ClusterMsg<U>>(name.clone()),
        hbr_dest: Destination::new::<HeartbeatReceiverMsg>(
          HeartbeatReceiver::<U>::from_clr(name.as_str(), id),
        ),
        subscribers: subrs,
        hb_interval_changes: 0,
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
