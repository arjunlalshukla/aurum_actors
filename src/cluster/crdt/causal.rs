#![allow(unused_imports, dead_code)]
use crate as aurum;
use crate::cluster::crdt::{DeltaMutator, CRDT};
use crate::cluster::{ClusterCmd, Member, UnifiedBounds, FAILURE_MODE};
use crate::core::{Actor, ActorContext, Case, Destination, LocalRef, Node};
use crate::testkit::FailureConfigMap;
use crate::{udp_select, AurumInterface};
use async_trait::async_trait;
use im;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;

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

pub struct DispersalPreference {
  priority: im::HashSet<Arc<Member>>,
  amount: u32,
  selector: DispersalSelector,
}

pub enum DispersalSelector {
  OutOfDate,
  //Random
}

pub struct CausalDisperse<S, U>
where
  U: UnifiedBounds + Case<CausalMsg<S>> + Case<CausalIntraMsg<S>>,
  S: CRDT,
{
  dest: Destination<U, CausalIntraMsg<S>>,
  fail_map: FailureConfigMap,
  subscribers: Vec<LocalRef<S>>,
  preference: DispersalPreference,
  cluster_ref: LocalRef<ClusterCmd>,

  state: S,
  clock: u64,
  member: Arc<Member>,
  cluster: im::HashSet<Arc<Member>>,
  deltas: VecDeque<S>,
  acks: im::HashMap<u64, (Arc<Member>, u64)>,
  ord_acks: im::Vector<(u64, u64)>,
  min_ord: u64,
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
  ) {
    let actor = Self {
      dest: Destination::new::<CausalMsg<S>>(name.clone()),
      fail_map: fail_map,
      subscribers: subscribers,
      preference: preference,
      cluster_ref: cluster_ref,
      state: S::minimum(),
      clock: 0,
      member: Arc::new(Member::default()),
      cluster: im::hashset!(),
      deltas: VecDeque::new(),
      acks: im::hashmap!(),
      ord_acks: im::vector!(),
      min_ord: 0,
    };
    node.spawn(false, actor, name, true);
  }

  async fn disperse(&mut self, ctx: &ActorContext<U, CausalMsg<S>>) {
    let to_send = match self.preference.selector {
      DispersalSelector::OutOfDate => self
        .ord_acks
        .iter()
        .map(|(_, id)| &self.acks.get(id).unwrap().0)
        .filter(|member| !self.preference.priority.contains(*member))
        .take(self.preference.amount as usize),
    };
    let delta = self.deltas.clone().into_iter().fold(S::minimum(), S::join);
    let msg = CausalIntraMsg::Delta(delta, self.member.id, self.clock);
    for member in self.preference.priority.iter().chain(to_send) {
      udp_select!(
        FAILURE_MODE,
        &ctx.node,
        &self.fail_map,
        &member.socket,
        &self.dest,
        &msg
      );
    }
  }

  fn ack(&mut self, id: u64, clock: u64) {
    let cnt = match self.acks.get_mut(&id) {
      Some((_, c)) if *c < clock => c,
      _ => return,
    };
    let idx = self.ord_acks.binary_search(&(*cnt, id)).unwrap();
    self.ord_acks.remove(idx);
    *cnt = clock;
    let new_min = self.ord_acks.front().unwrap().0;
    let to_remove = new_min - self.min_ord;
    for _ in 0..to_remove {
      self.deltas.pop_front().unwrap();
    }
    self.min_ord = new_min;
  }

  fn op(&mut self, op: S::Delta) {
    let d = op.apply(&self.state);
    self.state = self.state.clone().join(d.clone());
    self.deltas.push_back(d);
    self.clock += 1;
  }

  async fn delta(
    &mut self,
    ctx: &ActorContext<U, CausalMsg<S>>,
    delta: S,
    id: u64,
    clock: u64,
  ) {
    if let Some(socket) = self.acks.get(&id).map(|x| &x.0.socket) {
      let new_state = self.state.clone().join(delta.clone());
      if new_state != self.state {
        self.state = new_state;
        self.deltas.push_back(delta);
        self.clock += 1;
      }
      let msg = CausalIntraMsg::Ack {
        id: self.member.id,
        clock: clock,
      };
      udp_select!(
        FAILURE_MODE,
        &ctx.node,
        &self.fail_map,
        socket,
        &self.dest,
        &msg
      );
    }
  }
}
#[async_trait]
impl<U, S> Actor<U, CausalMsg<S>> for CausalDisperse<S, U>
where
  U: UnifiedBounds + Case<CausalMsg<S>> + Case<CausalIntraMsg<S>>,
  S: CRDT,
{
  async fn pre_start(&mut self, ctx: &ActorContext<U, CausalMsg<S>>) {
    //let subr = S
    //self.cluster_ref.send(ClusterCmd::Subscribe(ctx.local_interface()));
  }

  async fn recv(
    &mut self,
    ctx: &ActorContext<U, CausalMsg<S>>,
    msg: CausalMsg<S>,
  ) {
    match msg {
      CausalMsg::Cmd(cmd) => match cmd {
        CausalCmd::Mutate(op) => self.op(op),
        CausalCmd::Subscribe(subr) => {
          self.subscribers.push(subr);
        }
        CausalCmd::SetDispersalPreference(pref) => {
          self.preference = pref;
        }
      },
      CausalMsg::Intra(intra) => match intra {
        CausalIntraMsg::Ack { id, clock } => self.ack(id, clock),
        CausalIntraMsg::Delta(delta, id, clock) => {
          self.delta(ctx, delta, id, clock).await
        }
      },
    };
  }
}
