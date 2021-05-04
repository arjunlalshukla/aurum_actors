#![allow(unused_imports, dead_code)]
use crate as aurum;
use crate::cluster::crdt::{DeltaMutator, CRDT, LOG_LEVEL};
use crate::cluster::{
  ClusterCmd, ClusterEvent, ClusterUpdate, Member, NodeRing, UnifiedBounds,
  FAILURE_MODE,
};
use crate::core::{Actor, ActorContext, Case, Destination, LocalRef, Node};
use crate::testkit::FailureConfigMap;
use crate::{trace, udp_select, AurumInterface};
use async_trait::async_trait;
use im;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::iter::FromIterator;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

/*
This dispersal algorithm is based on algorithm 2 from
Delta State Replicated Data Types - Almeida, Shoker, Baquero
https://arxiv.org/pdf/1603.01529.pdf
*/

#[derive(AurumInterface)]
#[aurum(local)]
pub enum CausalMsg<S: CRDT> {
  #[aurum(local)]
  Cmd(CausalCmd<S>),
  #[aurum]
  Intra(CausalIntraMsg<S>),
  #[aurum(local)]
  Update(ClusterUpdate),
  DisperseTimeout,
}

#[derive(Serialize, Deserialize)]
#[serde(bound = "S: CRDT")]
pub enum CausalIntraMsg<S: CRDT> {
  Delta(S, u64, u64),
  Ack { id: u64, clock: u64 },
}

pub enum CausalCmd<S: CRDT> {
  Mutate(S::Delta),
  Subscribe(LocalRef<S>),
  SetDispersalPreference(DispersalPreference),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DispersalPreference {
  pub priority: im::HashSet<Arc<Member>>,
  pub amount: u32,
  pub selector: DispersalSelector,
  pub timeout: Duration,
}
impl Default for DispersalPreference {
  fn default() -> Self {
    Self {
      priority: im::HashSet::new(),
      amount: 1,
      selector: DispersalSelector::OutOfDate,
      timeout: Duration::from_millis(50),
    }
  }
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub enum DispersalSelector {
  OutOfDate,
  //Random
}

struct InCluster<S, U>
where
  U: UnifiedBounds + Case<CausalMsg<S>> + Case<CausalIntraMsg<S>>,
  S: CRDT,
{
  data: S,
  clock: u64,
  member: Arc<Member>,
  cluster: im::HashSet<Arc<Member>>,
  deltas: VecDeque<S>,
  acks: im::HashMap<u64, (Arc<Member>, u64)>,
  ord_acks: im::Vector<(u64, u64)>,
  min_ord: u64,
  x: PhantomData<U>,
}
impl<S, U> InCluster<S, U>
where
  U: UnifiedBounds + Case<CausalMsg<S>> + Case<CausalIntraMsg<S>>,
  S: CRDT,
{
  async fn disperse(
    &mut self,
    common: &Common<S, U>,
    ctx: &ActorContext<U, CausalMsg<S>>,
  ) {
    trace!(LOG_LEVEL, &ctx.node, format!("ORD_ACKS: {:?}", self.ord_acks));
    let to_send = match common.preference.selector {
      DispersalSelector::OutOfDate => self
        .ord_acks
        .iter()
        .map(|(_, id)| &self.acks.get(id).unwrap().0)
        .filter(|member| !common.preference.priority.contains(*member))
        .take(common.preference.amount as usize),
    };
    let delta = self.deltas.clone().into_iter().fold(S::minimum(), S::join);
    let msg = CausalIntraMsg::Delta(delta, self.member.id, self.clock);
    let members = common
      .preference
      .priority
      .iter()
      .chain(to_send)
      .collect_vec();
    trace!(
      LOG_LEVEL,
      &ctx.node,
      format!(
        "Dispersing to {:?}",
        members.iter().map(|m| m.socket.udp).collect_vec()
      )
    );
    for member in members.iter().cloned() {
      udp_select!(
        FAILURE_MODE,
        &ctx.node,
        &common.fail_map,
        &member.socket,
        &common.dest,
        &msg
      );
    }
    ctx.node.schedule_local_msg(
      common.preference.timeout,
      ctx.local_interface(),
      CausalMsg::DisperseTimeout,
    );
  }

  fn ack(&mut self, id: u64, clock: u64) {
    let cnt = match self.acks.get_mut(&id) {
      Some((_, c)) if *c < clock => c,
      _ => return,
    };
    let idx = self.ord_acks.binary_search(&(*cnt, id)).unwrap();
    self.ord_acks.remove(idx);
    *cnt = clock;
    self.ord_acks.insert_ord((*cnt, id));
    let new_min = self.ord_acks.front().unwrap().0;
    let to_remove = new_min - self.min_ord;
    for _ in 0..to_remove {
      self.deltas.pop_front().unwrap();
    }
    self.min_ord = new_min;
  }

  async fn op(
    &mut self,
    common: &mut Common<S, U>,
    ctx: &ActorContext<U, CausalMsg<S>>,
    op: S::Delta,
  ) {
    let d = op.apply(&self.data);
    if !d.empty() {
      self.data = self.data.clone().join(d.clone());
      self.deltas.push_back(d);
      self.clock += 1;
      self.disperse(common, ctx).await;
      self.publish(common);    
    }
  }

  async fn delta(
    &mut self,
    common: &mut Common<S, U>,
    ctx: &ActorContext<U, CausalMsg<S>>,
    delta: S,
    id: u64,
    clock: u64,
  ) {
    if let Some(socket) = self.acks.get(&id).map(|x| &x.0.socket) {
      let msg = CausalIntraMsg::Ack {
        id: self.member.id,
        clock: clock,
      };
      trace!(
        LOG_LEVEL,
        &ctx.node,
        format!("Got delta from {:?}", socket.udp)
      );
      udp_select!(
        FAILURE_MODE,
        &ctx.node,
        &common.fail_map,
        socket,
        &common.dest,
        &msg
      );
      let new_state = self.data.clone().join(delta.clone());
      if new_state != self.data {
        self.data = new_state;
        self.deltas.push_back(delta);
        self.clock += 1;
        self.disperse(common, ctx).await;
        self.publish(common);
      }
    }
  }

  fn update(&mut self, ctx: &ActorContext<U, CausalMsg<S>>, update: ClusterUpdate) {
    trace!(LOG_LEVEL, &ctx.node, format!("Got cluster update"));
    for event in update.events {
      match event {
        ClusterEvent::Alone(m) => self.member = m,
        ClusterEvent::Joined(m) => self.member = m,
        ClusterEvent::Removed(m) => {
          if m.id != self.member.id {
            let cnt = self.acks.remove(&m.id).unwrap().1;
            let idx = self.ord_acks.binary_search(&(cnt, m.id)).unwrap();
            self.ord_acks.remove(idx);
            self.min_ord = self.ord_acks.front().map(|(c, _)| *c).unwrap_or(0);
          }
        }
        ClusterEvent::Added(m) => {
          self.ord_acks.insert_ord((0, m.id));
          self.acks.insert(m.id, (m, 0));
          self.min_ord = 0;
        }
        ClusterEvent::Left => unreachable!(),
      }
    }
    self.cluster = update.nodes;
    self.cluster.remove(&self.member);
    trace!(LOG_LEVEL, &ctx.node, format!("Finished cluster update"));
  }

  fn publish(&self, common: &mut Common<S, U>) {
    common
      .subscribers
      .retain(|subr| subr.send(self.data.clone()));
  }
}

struct Waiting<S: CRDT> {
  ops_queue: Vec<S::Delta>,
}
impl<S: CRDT> Waiting<S> {
  fn to_ic<U: UnifiedBounds + Case<CausalMsg<S>> + Case<CausalIntraMsg<S>>>(
    &self,
    member: Arc<Member>,
    mut nodes: im::HashSet<Arc<Member>>,
  ) -> InCluster<S, U> {
    let init_data = self.ops_queue.iter().fold(S::minimum(), |data, op| {
      let delta = op.apply(&data);
      data.join(delta)
    });
    nodes.remove(&member);
    InCluster {
      data: init_data.clone(),
      clock: 1,
      deltas: VecDeque::from(vec![init_data]),
      member: member,
      acks: nodes.iter().map(|m| (m.id, (m.clone(), 0))).collect(),
      ord_acks: nodes.iter().map(|m| (0, m.id)).sorted().collect(),
      cluster: nodes,
      min_ord: 0,
      x: PhantomData,
    }
  }
}

enum InteractionState<S, U>
where
  U: UnifiedBounds + Case<CausalMsg<S>> + Case<CausalIntraMsg<S>>,
  S: CRDT,
{
  InCluster(InCluster<S, U>),
  Waiting(Waiting<S>),
}

struct Common<S, U>
where
  U: UnifiedBounds + Case<CausalMsg<S>> + Case<CausalIntraMsg<S>>,
  S: CRDT,
{
  dest: Destination<U, CausalIntraMsg<S>>,
  fail_map: FailureConfigMap,
  subscribers: Vec<LocalRef<S>>,
  preference: DispersalPreference,
  cluster_ref: LocalRef<ClusterCmd>,
}

pub struct CausalDisperse<S, U>
where
  U: UnifiedBounds + Case<CausalMsg<S>> + Case<CausalIntraMsg<S>>,
  S: CRDT,
{
  common: Common<S, U>,
  state: InteractionState<S, U>,
}
impl<S, U> CausalDisperse<S, U>
where
  U: UnifiedBounds + Case<CausalMsg<S>> + Case<CausalIntraMsg<S>>,
  S: CRDT,
{
  pub fn new(
    node: &Node<U>,
    name: String,
    fail_map: FailureConfigMap,
    subscribers: Vec<LocalRef<S>>,
    preference: DispersalPreference,
    cluster_ref: LocalRef<ClusterCmd>,
  ) -> LocalRef<CausalCmd<S>> {
    let actor = Self {
      common: Common {
        dest: Destination::new::<CausalMsg<S>>(name.clone()),
        fail_map: fail_map,
        subscribers: subscribers,
        preference: preference,
        cluster_ref: cluster_ref,
      },
      state: InteractionState::Waiting(Waiting { ops_queue: vec![] }),
    };
    node
      .spawn(false, actor, name, true)
      .local()
      .clone()
      .unwrap()
      .transform()
  }
}
#[async_trait]
impl<U, S> Actor<U, CausalMsg<S>> for CausalDisperse<S, U>
where
  U: UnifiedBounds + Case<CausalMsg<S>> + Case<CausalIntraMsg<S>>,
  S: CRDT,
{
  async fn pre_start(&mut self, ctx: &ActorContext<U, CausalMsg<S>>) {
    self
      .common
      .cluster_ref
      .send(ClusterCmd::Subscribe(ctx.local_interface()));
  }

  async fn recv(
    &mut self,
    ctx: &ActorContext<U, CausalMsg<S>>,
    msg: CausalMsg<S>,
  ) {
    match msg {
      CausalMsg::Cmd(cmd) => match cmd {
        CausalCmd::Mutate(op) => match &mut self.state {
          InteractionState::InCluster(ic) => {
            ic.op(&mut self.common, ctx, op).await
          }
          InteractionState::Waiting(w) => w.ops_queue.push(op),
        },
        CausalCmd::Subscribe(subr) => {
          self.common.subscribers.push(subr);
        }
        CausalCmd::SetDispersalPreference(pref) => {
          self.common.preference = pref;
        }
      },
      CausalMsg::Intra(intra) => {
        if let InteractionState::InCluster(ic) = &mut self.state {
          match intra {
            CausalIntraMsg::Ack { id, clock } => ic.ack(id, clock),
            CausalIntraMsg::Delta(delta, id, clock) => {
              ic.delta(&mut self.common, ctx, delta, id, clock).await
            }
          }
        }
      }
      CausalMsg::Update(update) => match &mut self.state {
        InteractionState::InCluster(ic) => ic.update(ctx, update),
        InteractionState::Waiting(w) => {
          let ic = match update.events.into_iter().next().unwrap() {
            ClusterEvent::Alone(m) => w.to_ic(m, update.nodes),
            ClusterEvent::Joined(m) => w.to_ic(m, update.nodes),
            _ => unreachable!(),
          };
          self.state = InteractionState::InCluster(ic);
        }
      },
      CausalMsg::DisperseTimeout => {
        if let InteractionState::InCluster(ic) = &mut self.state {
          if ic.min_ord != ic.clock && !ic.acks.is_empty() {
            ic.disperse(&self.common, ctx).await;
          }
        }
      }
    };
  }
}
