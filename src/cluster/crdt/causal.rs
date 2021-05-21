use crate as aurum;
use crate::cluster::crdt::{DeltaMutator, CRDT, LOG_LEVEL};
use crate::cluster::{
  ClusterCmd, ClusterEvent, ClusterUpdate, Member, FAILURE_MODE,
};
use crate::core::{
  Actor, ActorContext, Case, Destination, LocalRef, Node, UnifiedType,
};
use crate::testkit::FailureConfigMap;
use crate::{debug, trace, udp_select, AurumInterface};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, VecDeque};
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
  U: UnifiedType + Case<CausalMsg<S>> + Case<CausalIntraMsg<S>>,
  S: CRDT,
{
  data: S,
  clock: u64,
  member: Arc<Member>,
  cluster: im::HashSet<Arc<Member>>,
  deltas: VecDeque<S>,
  acks: im::HashMap<u64, (Arc<Member>, u64)>,
  ord_acks: BTreeSet<(u64, u64)>,
  min_ord: u64,
  min_delta: u64,
  x: PhantomData<U>,
}
impl<S, U> InCluster<S, U>
where
  U: UnifiedType + Case<CausalMsg<S>> + Case<CausalIntraMsg<S>>,
  S: CRDT,
{
  async fn disperse(
    &mut self,
    common: &Common<S, U>,
    ctx: &ActorContext<U, CausalMsg<S>>,
  ) {
    trace!(
      LOG_LEVEL,
      &ctx.node,
      format!("CLOCK: {} ORD_ACKS: {:?}", self.clock, self.ord_acks)
    );
    let to_send = match common.preference.selector {
      DispersalSelector::OutOfDate => self
        .ord_acks
        .iter()
        .map(|(_, id)| self.acks.get(id).unwrap())
        .filter(|(member, _)| !common.preference.priority.contains(member))
        .take(common.preference.amount as usize),
    };
    let delta = self.deltas.clone().into_iter().fold(S::minimum(), S::join);
    let delta_msg = CausalIntraMsg::Delta(delta, self.member.id, self.clock);
    let full_msg =
      CausalIntraMsg::Delta(self.data.clone(), self.member.id, self.clock);
    let members = common
      .preference
      .priority
      .iter()
      .map(|m| self.acks.get(&m.id).unwrap())
      .chain(to_send)
      .filter(|(_, cnt)| *cnt < self.clock);
    for (member, cnt) in members {
      let p = member.socket.udp;
      let msg = if *cnt < self.min_delta {
        trace!(LOG_LEVEL, &ctx.node, format!("full data to {}", p));
        &full_msg
      } else {
        trace!(LOG_LEVEL, &ctx.node, format!("delta to {}", p));
        &delta_msg
      };
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

  fn ack(&mut self, ctx: &ActorContext<U, CausalMsg<S>>, id: u64, clock: u64) {
    let (cnt, member) = match self.acks.get_mut(&id) {
      Some((m, c)) if *c < clock => (c, m),
      _ => return,
    };
    debug!(
      LOG_LEVEL,
      &ctx.node,
      format!(
        "CLOCK: {} - ACK({}) from {}",
        self.clock, clock, member.socket.udp
      )
    );
    assert!(self.ord_acks.remove(&(*cnt, id)));
    *cnt = clock;
    self.ord_acks.insert((*cnt, id));
    self.min_ord = self.ord_acks.iter().next().unwrap().0;
    for _ in self.min_delta..self.min_ord {
      self.deltas.pop_front().unwrap();
    }
    self.min_delta = std::cmp::max(self.min_delta, self.min_ord);
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
        debug!(
          LOG_LEVEL,
          &ctx.node,
          format!("CLOCK: {} - Got delta from {:?}", self.clock, socket.udp)
        );
        self.disperse(common, ctx).await;
        self.publish(common);
      }
    }
  }

  async fn update(
    &mut self,
    common: &mut Common<S, U>,
    ctx: &ActorContext<U, CausalMsg<S>>,
    update: ClusterUpdate,
  ) {
    let mut disperse = false;
    for event in update.events {
      match event {
        ClusterEvent::Alone(m) => self.member = m,
        ClusterEvent::Joined(m) => self.member = m,
        ClusterEvent::Removed(m) => {
          if m.id != self.member.id {
            let cnt = self.acks.remove(&m.id).unwrap().1;
            assert!(self.ord_acks.remove(&(cnt, m.id)));
            self.min_ord =
              self.ord_acks.iter().next().map(|(c, _)| *c).unwrap_or(0);
            disperse = true;
          }
        }
        ClusterEvent::Added(m) => {
          self.ord_acks.insert((0, m.id));
          self.acks.insert(m.id, (m, 0));
          self.min_ord = 0;
          disperse = true;
        }
        ClusterEvent::Left => unreachable!(),
      }
    }
    self.cluster = update.nodes;
    self.cluster.remove(&self.member);
    if disperse {
      self.disperse(common, ctx).await;
    }
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
  fn to_ic<U: UnifiedType + Case<CausalMsg<S>> + Case<CausalIntraMsg<S>>>(
    &self,
    member: Arc<Member>,
    mut nodes: im::HashSet<Arc<Member>>,
  ) -> InCluster<S, U> {
    let init_data = self.ops_queue.iter().fold(S::minimum(), |data, op| {
      let delta = op.apply(&data);
      data.join(delta)
    });
    nodes.remove(&member);
    let mut deltas = VecDeque::new();
    if !init_data.empty() {
      deltas.push_back(init_data.clone());
    }
    InCluster {
      clock: !init_data.empty() as u64,
      data: init_data,
      deltas: deltas,
      member: member,
      acks: nodes.iter().map(|m| (m.id, (m.clone(), 0))).collect(),
      ord_acks: nodes.iter().map(|m| (0, m.id)).collect(),
      cluster: nodes,
      min_ord: 0,
      min_delta: 0,
      x: PhantomData,
    }
  }
}

enum State<S, U>
where
  U: UnifiedType + Case<CausalMsg<S>> + Case<CausalIntraMsg<S>>,
  S: CRDT,
{
  InCluster(InCluster<S, U>),
  Waiting(Waiting<S>),
}

struct Common<S, U>
where
  U: UnifiedType + Case<CausalMsg<S>> + Case<CausalIntraMsg<S>>,
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
  U: UnifiedType + Case<CausalMsg<S>> + Case<CausalIntraMsg<S>>,
  S: CRDT,
{
  common: Common<S, U>,
  state: State<S, U>,
}
impl<S, U> CausalDisperse<S, U>
where
  U: UnifiedType + Case<CausalMsg<S>> + Case<CausalIntraMsg<S>>,
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
      state: State::Waiting(Waiting { ops_queue: vec![] }),
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
  U: UnifiedType + Case<CausalMsg<S>> + Case<CausalIntraMsg<S>>,
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
          State::InCluster(ic) => ic.op(&mut self.common, ctx, op).await,
          State::Waiting(w) => w.ops_queue.push(op),
        },
        CausalCmd::Subscribe(subr) => {
          if let State::InCluster(ic) = &self.state {
            subr.send(ic.data.clone());
          }
          self.common.subscribers.push(subr);
        }
        CausalCmd::SetDispersalPreference(pref) => {
          self.common.preference = pref;
        }
      },
      CausalMsg::Intra(intra) => {
        if let State::InCluster(ic) = &mut self.state {
          match intra {
            CausalIntraMsg::Ack { id, clock } => ic.ack(ctx, id, clock),
            CausalIntraMsg::Delta(delta, id, clock) => {
              ic.delta(&mut self.common, ctx, delta, id, clock).await
            }
          }
        }
      }
      CausalMsg::Update(mut update) => match &mut self.state {
        State::InCluster(ic) => {
          ic.update(&mut self.common, ctx, update).await;
        }
        State::Waiting(w) => {
          let ic = match update.events.pop().unwrap() {
            ClusterEvent::Alone(m) => w.to_ic(m, update.nodes),
            ClusterEvent::Joined(m) => w.to_ic(m, update.nodes),
            _ => unreachable!(),
          };
          ic.publish(&mut self.common);
          self.state = State::InCluster(ic);
        }
      },
      CausalMsg::DisperseTimeout => {
        if let State::InCluster(ic) = &mut self.state {
          if ic.min_ord != ic.clock && !ic.acks.is_empty() {
            ic.disperse(&self.common, ctx).await;
          }
        }
      }
    };
  }
}
