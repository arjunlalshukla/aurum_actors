use crate as aurum;
use crate::cluster::{
  ClusterConfig, ClusterEvent, ClusterUpdate, Gossip, HBRConfig,
  HeartbeatReceiver, HeartbeatReceiverMsg, MachineState, Member, NodeRing,
  UnifiedBounds, FAILURE_MODE, LOG_LEVEL,
};
use crate::core::{
  Actor, ActorContext, ActorRef, ActorSignal, Destination, LocalRef, Node,
};
use crate::testkit::FailureConfigMap;
use crate::{debug, info, trace, udp_select, AurumInterface};
use async_trait::async_trait;
use im;
use itertools::Itertools;
use maplit::{btreemap, hashset};
use rand::seq::IteratorRandom;
use serde::{Deserialize, Serialize};
use std::collections::btree_map::Entry;
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
  HeartbeatTick,
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
  Subscribe(LocalRef<ClusterUpdate>),
  FailureMap(FailureConfigMap),
}

struct InCluster {
  charges: HashMap<Arc<Member>, LocalRef<HeartbeatReceiverMsg>>,
  managers: Vec<Arc<Member>>,
  gossip: Gossip,
  members: im::HashSet<Arc<Member>>,
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
      managers: Vec::new(),
      gossip: Gossip {
        states: btreemap! {common.member.clone() => Up},
      },
      members: im::hashset![common.member.clone()],
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
      Foo(_) => None,
      ReqGossip(member) => {
        debug!(
          LOG_LEVEL,
          ctx.node,
          format!("got gossip req from {}", member.socket.udp)
        );
        let msg: IntraClusterMsg<U> = State(self.gossip.clone());
        udp_select!(
          FAILURE_MODE,
          &ctx.node,
          &common.fail_map,
          &member.socket,
          &common.clr_dest,
          &msg
        );
        None
      }
      ReqHeartbeat(member, id) => {
        // TODO: What if requester is not the manager?
        // For now, send heartbeat anyway. Conflicts will reconcile eventually.
        if id == common.member.id {
          common.heartbeat(ctx, &member).await;
        } else {
          debug!(
            LOG_LEVEL,
            ctx.node,
            format!(
              "got HB request for id {} when id is {}",
              common.member.id, id
            )
          );
        }
        None
      }
      State(gossip) => {
        self.gossip_timeout.abort();
        let events = self.gossip.merge(gossip);
        let disperse = !events.is_empty();
        for e in &events {
          match e {
            ClusterEvent::Added(member) => {
              self.ring.insert(member.clone());
              self.members.insert(member.clone());
            }
            ClusterEvent::Removed(member) => {
              if self.ring.remove(&*member).is_err()
                || self.members.remove(member).is_none()
              {
                trace!(
                  LOG_LEVEL,
                  ctx.node,
                  format!("failed to remove {:?}", member)
                );
              }
              if *member == common.member {
                common.new_id(ctx);
                self.ring.insert(common.member.clone());
                self.members.insert(member.clone());
                self.gossip.states.insert(common.member.clone(), Up);
              }
            }
            _ => {}
          }
        }
        if disperse {
          self.gossip_round(common, ctx, hashset!()).await;
        }
        self.update_charges_managers(common, ctx);
        self.notify(common, events);
        self.gossip_timeout = common.schedule_gossip_timeout(ctx);
        None
      }
      Ping(member) => {
        info!(
          LOG_LEVEL,
          ctx.node,
          format!("received ping from {:?}", member)
        );
        if let Entry::Vacant(v) = self.gossip.states.entry(member.clone()) {
          v.insert(Up);
          self.ring.insert(member.clone());
          self.members.insert(member.clone());
          self.update_charges_managers(common, ctx);
          self.notify(common, vec![ClusterEvent::Added(member.clone())]);
        } else {
          info!(LOG_LEVEL, ctx.node, format!("pinger already exists"));
        }
        self.gossip_round(common, ctx, hashset!(&member)).await;
        None
      }
    }
  }

  fn update_charges_managers<U: UnifiedBounds>(
    &mut self,
    common: &NodeState<U>,
    ctx: &ActorContext<U, ClusterMsg<U>>,
  ) {
    trace!(
      LOG_LEVEL,
      ctx.node,
      format!(
        "old charges: {:?}",
        self
          .charges
          .keys()
          .map(|m| (m.socket.udp, m.id))
          .collect_vec()
      )
    );
    let mut new_charges = HashMap::new();
    for member in self.ring.charges(&common.member).unwrap() {
      let hbr = self.charges.remove(&member).unwrap_or_else(|| {
        HeartbeatReceiver::spawn(ctx, common, member.clone())
      });
      new_charges.insert(member, hbr);
    }
    for (_, hbr) in self.charges.iter() {
      hbr.signal(ActorSignal::Term);
    }
    self.charges = new_charges;
    trace!(
      LOG_LEVEL,
      ctx.node,
      format!(
        "new charges: {:?}",
        self
          .charges
          .keys()
          .map(|m| (m.socket.udp, m.id))
          .collect_vec()
      )
    );
    trace!(
      LOG_LEVEL,
      ctx.node,
      format!(
        "old managers: {:?}",
        self
          .managers
          .iter()
          .map(|m| (m.socket.udp, m.id))
          .collect_vec()
      )
    );
    self.managers = self.ring.node_managers(&common.member).unwrap();
    trace!(
      LOG_LEVEL,
      ctx.node,
      format!(
        "old managers: {:?}",
        self
          .managers
          .iter()
          .map(|m| (m.socket.udp, m.id))
          .collect_vec()
      )
    );
  }

  async fn gossip_round<U: UnifiedBounds>(
    &self,
    common: &NodeState<U>,
    ctx: &ActorContext<U, ClusterMsg<U>>,
    mut guaranteed: HashSet<&Arc<Member>>,
  ) {
    let msg: IntraClusterMsg<U> = State(self.gossip.clone());
    self.managers.iter().for_each(|m| {
      guaranteed.insert(m);
    });
    self.charges.keys().for_each(|m| {
      guaranteed.insert(m);
    });
    guaranteed.insert(&common.member);
    self
      .members
      .iter()
      .filter(|m| !guaranteed.contains(*m))
      .choose_multiple(
        &mut rand::thread_rng(),
        common.clr_config.gossip_disperse,
      )
      .into_iter()
      .for_each(|m| {
        guaranteed.insert(m);
      });
    guaranteed.remove(&common.member);
    debug!(
      LOG_LEVEL,
      ctx.node,
      format!(
        "gossiping to {:?}",
        guaranteed.iter().map(|m| m.socket.udp).collect_vec()
      )
    );
    for member in guaranteed {
      udp_select!(
        FAILURE_MODE,
        &ctx.node,
        &common.fail_map,
        &member.socket,
        &common.clr_dest,
        &msg
      )
    }
  }

  async fn gossip_reqs<U: UnifiedBounds>(
    &self,
    common: &NodeState<U>,
    ctx: &ActorContext<U, ClusterMsg<U>>,
  ) {
    let msg: IntraClusterMsg<U> = ReqGossip(common.member.clone());
    debug!(
      LOG_LEVEL,
      ctx.node,
      format!(
        "gossip state: {:?}",
        self
          .gossip
          .states
          .iter()
          .map(|(m, s)| (m.socket.udp, s))
          .collect_vec()
      )
    );
    let recipients = self
      .members
      .iter()
      .filter(|m| (**m) != common.member)
      .choose_multiple(
        &mut rand::thread_rng(),
        common.clr_config.gossip_disperse,
      )
      .into_iter()
      .collect_vec();
    debug!(
      LOG_LEVEL,
      ctx.node,
      format!(
        "gossip reqs sent to: {:?}",
        recipients.iter().map(|m| m.socket.udp).collect_vec()
      )
    );
    for member in recipients {
      udp_select!(
        FAILURE_MODE,
        &ctx.node,
        &common.fail_map,
        &member.socket,
        &common.clr_dest,
        &msg
      )
    }
  }

  async fn down<U: UnifiedBounds>(
    &mut self,
    common: &mut NodeState<U>,
    ctx: &ActorContext<U, ClusterMsg<U>>,
    member: Arc<Member>,
  ) {
    let state = self.gossip.states.get_mut(&member).unwrap();
    if *state == Up {
      *state = Down;
      self.ring.remove(&member).unwrap();
      self.members.remove(&member).unwrap();
      self.update_charges_managers(common, ctx);
      self.gossip_round(common, ctx, hashset!(&member)).await;
      self.notify(common, vec![ClusterEvent::Removed(member)]);
    }
  }

  fn notify<U: UnifiedBounds>(
    &self,
    common: &mut NodeState<U>,
    events: Vec<ClusterEvent>,
  ) {
    let msg  = ClusterUpdate {
      events: events,
      nodes: self.members.clone(),
      ring: self.ring.clone(),
    };
    common.subscribers.retain(|s| s.send(msg.clone()));
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
    info!(
      LOG_LEVEL,
      ctx.node,
      format!("starting ping. {} attempts left", self.count)
    );
    let msg: IntraClusterMsg<U> = Ping(common.member.clone());
    for s in common.clr_config.seed_nodes.iter() {
      udp_select!(
        FAILURE_MODE,
        &ctx.node,
        &common.fail_map,
        s,
        &common.clr_dest,
        &msg
      );
    }
    let ar = ctx.local_interface();
    self.timeout =
      ctx.node.schedule(common.clr_config.ping_timeout, move || {
        ar.send(ClusterMsg::PingTimeout);
      });
  }

  async fn process<U: UnifiedBounds>(
    &mut self,
    common: &mut NodeState<U>,
    ctx: &ActorContext<U, ClusterMsg<U>>,
    msg: IntraClusterMsg<U>,
  ) -> Option<InteractionState> {
    match msg {
      // Getting this message means the response to our ping wasn't received.
      ReqHeartbeat(member, id) => {
        debug!(LOG_LEVEL, ctx.node, format!("Got HB request while pinging"));
        if id == common.member.id {
          common.heartbeat(ctx, &member).await;
        } else {
          debug!(
            LOG_LEVEL,
            ctx.node,
            format!(
              "Got HB request for id {} when id is {}",
              common.member.id, id
            )
          );
        }
        udp_select!(
          FAILURE_MODE,
          &ctx.node,
          &common.fail_map,
          &member.socket,
          &common.clr_dest,
          &ReqGossip(common.member.clone())
        );
        None
      }
      State(mut gossip) => {
        let me = gossip.states.get(&common.member);
        let downed = me.filter(|s| s >= &&Down).is_some();
        if downed {
          common.new_id(ctx);
        }
        if me.is_none() || downed {
          gossip.states.insert(common.member.clone(), Up);
        }
        let mut ring = NodeRing::new(common.clr_config.replication_factor);
        let mut members = im::HashSet::new();
        gossip
          .states
          .iter()
          .filter(|(_, s)| s < &&Down)
          .map(|(m, _)| m.clone())
          .for_each(|member| {
            ring.insert(member.clone());
            members.insert(member);
          });
        let mut ic = InCluster {
          charges: HashMap::new(),
          managers: Vec::new(),
          gossip: gossip,
          members: members,
          ring: ring,
          gossip_timeout: common.schedule_gossip_timeout(ctx),
        };
        ic.update_charges_managers(common, ctx);
        ic.gossip_round(common, ctx, hashset!()).await;
        ic.notify(common, vec![ClusterEvent::Joined(common.member.clone())]);
        debug!(
          LOG_LEVEL,
          ctx.node,
          format!(
            "responsible for {:?}",
            ic.charges
              .keys()
              .map(|m| (m.socket.udp, m.id))
              .collect_vec()
          )
        );
        Some(InteractionState::InCluster(ic))
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
  pub subscribers: Vec<LocalRef<ClusterUpdate>>,
  pub fail_map: FailureConfigMap,
  pub hb_interval_changes: u32,
  pub clr_config: ClusterConfig,
  pub hbr_config: HBRConfig,
}
impl<U: UnifiedBounds> NodeState<U> {
  fn new_id(&mut self, ctx: &ActorContext<U, ClusterMsg<U>>) {
    let old_id = self.member.id;
    self.member = Arc::new(Member {
      socket: self.member.socket.clone(),
      id: rand::random(),
      vnodes: self.member.vnodes,
    });
    self.hbr_dest = Destination::new::<HeartbeatReceiverMsg>(
      HeartbeatReceiver::<U>::from_clr(
        self.clr_dest.name().name.as_str(),
        self.member.id,
      ),
    );
    info!(
      LOG_LEVEL,
      ctx.node,
      format!(
        "I've been downed, changing id from {} to {}",
        old_id, self.member.id
      )
    );
  }

  async fn heartbeat(
    &self,
    ctx: &ActorContext<U, ClusterMsg<U>>,
    member: &Member,
  ) {
    let msg = HeartbeatReceiverMsg::Heartbeat(
      self.clr_config.hb_interval,
      self.hb_interval_changes,
    );
    udp_select!(
      FAILURE_MODE,
      &ctx.node,
      &self.fail_map,
      &member.socket,
      &self.hbr_dest,
      &msg
    );
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
}

pub struct Cluster<U: UnifiedBounds> {
  common: NodeState<U>,
  state: InteractionState,
}
#[async_trait]
impl<U: UnifiedBounds> Actor<U, ClusterMsg<U>> for Cluster<U> {
  async fn pre_start(&mut self, ctx: &ActorContext<U, ClusterMsg<U>>) {
    info!(
      LOG_LEVEL,
      ctx.node,
      format!("STARTING member with id: {}", self.common.member.id)
    );
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
    ctx.node.schedule_local_msg(
      self.common.clr_config.hb_interval,
      ctx.local_interface(),
      ClusterMsg::HeartbeatTick,
    );
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
        if let InteractionState::InCluster(ic) = &self.state {
          let nodes = ic.members.clone();
          let ring = ic.ring.clone();
          let event = if nodes.len() == 1 {
            ClusterEvent::Alone(self.common.member.clone())
          } else {
            ClusterEvent::Joined(self.common.member.clone())
          };
          subr.send(ClusterUpdate {
            events: vec![event],
            nodes: nodes,
            ring: ring,
          });
        }
        self.common.subscribers.push(subr);
      }
      ClusterMsg::LocalCmd(ClusterCmd::FailureMap(map)) => {
        self.common.fail_map = map;
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
          ic.gossip_round(&self.common, ctx, hashset!()).await;
          ic.gossip_reqs(&self.common, ctx).await;
          self.common.schedule_gossip_timeout(ctx);
        }
      }
      ClusterMsg::Downed(member) => {
        if let InteractionState::InCluster(ic) = &mut self.state {
          ic.down(&mut self.common, ctx, member).await;
        }
      }
      ClusterMsg::HeartbeatTick => {
        if let InteractionState::InCluster(ic) = &mut self.state {
          for member in ic.managers.iter() {
            self.common.heartbeat(ctx, member).await;
          }
        }
        ctx.node.schedule_local_msg(
          self.common.clr_config.hb_interval,
          ctx.local_interface(),
          ClusterMsg::HeartbeatTick,
        );
      }
    }
  }
}
impl<U: UnifiedBounds> Cluster<U> {
  pub async fn new(
    node: &Node<U>,
    name: String,
    vnodes: u32,
    subrs: Vec<LocalRef<ClusterUpdate>>,
    fail_map: FailureConfigMap,
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
        fail_map: fail_map,
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
    let ic = InCluster::alone(&self.common, ctx);
    let mem = self.common.member.clone();
    ic.notify(&mut self.common, vec![ClusterEvent::Alone(mem)]);
    self.state = InteractionState::InCluster(ic);
  }
}
