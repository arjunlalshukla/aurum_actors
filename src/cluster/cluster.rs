//#![allow(unused_imports, dead_code, unused_variables)]

use crate as aurum;
use crate::cluster::{
  ClusterConfig, ClusterEvent, Gossip, HBRConfig, HeartbeatReceiver,
  HeartbeatReceiverMsg, MachineState, Member, NodeRing, UnifiedBounds,
  RELIABLE,
};
use crate::core::{
  Actor, ActorContext, ActorRef, ActorSignal, Destination, LocalRef, Node,
};
use crate::{udp_send, AurumInterface};
use async_trait::async_trait;
use itertools::Itertools;
use maplit::btreemap;
use rand::seq::IteratorRandom;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::task::JoinHandle;
use IntraClusterMsg::*;
use MachineState::*;

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

struct InCluster {
  charges: HashMap<Arc<Member>, LocalRef<HeartbeatReceiverMsg>>,
  gossip: Gossip,
  ring: NodeRing,
  gossip_timeout: JoinHandle<bool>,
}
impl InCluster {
  fn alone<U: UnifiedBounds>(
    common: &NodeState<U>,
    ctx: &ActorContext<U, ClusterMsg<U>>,
  ) -> InCluster {
    let mut ring = NodeRing::new(common.clr_config.replication_factor);
    ring.insert(common.member.clone());
    InCluster {
      charges: HashMap::new(),
      gossip: Gossip {
        states: btreemap! {common.member.clone() => Up},
      },
      ring: ring,
      gossip_timeout: ctx.node.rt().spawn(async { true }),
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
      ReqGossip(member) => {
        let msg: IntraClusterMsg<U> = State(self.gossip.clone());
        udp_send!(RELIABLE, &member.socket, &common.clr_dest, &msg)
      }
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
        self.gossip_timeout.abort();
        let events = self.gossip.merge(gossip);
        if !events.is_empty() {
          common.gossip_round(&self.gossip, HashSet::new()).await;
        }
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
        self.gossip_timeout = common.schedule_gossip_timeout(ctx);
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
          gossip_timeout: common.schedule_gossip_timeout(ctx),
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

  fn schedule_gossip_timeout(
    &self,
    ctx: &ActorContext<U, ClusterMsg<U>>,
  ) -> JoinHandle<bool> {
    ctx.node.schedule_local_msg(
      self.clr_config.gossip_timeout,
      ctx.local_interface(),
      ClusterMsg::GossipTimeout,
    )
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
      .chain(guaranteed.into_iter())
      .collect_vec();
    println!(
      "{}: gossiping to {:?}",
      self.member.socket.udp,
      members.iter().map(|m| m.socket.udp).collect_vec()
    );
    for member in members {
      udp_send!(RELIABLE, &member.socket, &self.clr_dest, &msg)
    }
  }

  async fn gossip_reqs(&self, gossip: &Gossip) {
    let msg: IntraClusterMsg<U> = ReqGossip(self.member.clone());
    let members = gossip
      .states
      .iter()
      .filter(|(m, s)| (**m) != self.member && s < &&Down)
      .map(|(m, _)| m)
      .choose_multiple(&mut rand::thread_rng(), self.clr_config.gossip_disperse)
      .into_iter();
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
      self.create_cluster(ctx);
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
            self.create_cluster(ctx);
          }
        }
      }
      ClusterMsg::GossipTimeout => {
        if let InteractionState::InCluster(ic) = &self.state {
          println!("{}: gossip wait timed out", self.common.member.socket.udp);
          self.common.gossip_round(&ic.gossip, HashSet::new()).await;
          self.common.gossip_reqs(&ic.gossip).await;
          ctx.node.schedule_local_msg(
            self.common.clr_config.gossip_timeout,
            ctx.local_interface(),
            ClusterMsg::GossipTimeout,
          );
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

  fn create_cluster(&mut self, ctx: &ActorContext<U, ClusterMsg<U>>) {
    self
      .common
      .notify(ClusterEvent::Alone(self.common.member.clone()));
    self.state =
      InteractionState::InCluster(InCluster::alone(&self.common, ctx));
  }
}
