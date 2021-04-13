#![allow(unused_imports, dead_code, unused_variables)]

use crate as aurum;
use crate::cluster::{
  ClusterMsg, IntervalStorage, Member, NodeState, UnifiedBounds,
};
use crate::core::{ActorContext, Case, LocalRef, Node, TimeoutActor};
use crate::AurumInterface;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use HeartbeatReceiverMsg::*;

#[derive(Clone)]
pub struct HBRConfig {
  pub phi: f64,
  pub capacity: usize,
  pub times: usize,
  pub req_tries: usize,
  pub req_timeout: Duration,
}
impl Default for HBRConfig {
  fn default() -> Self {
    HBRConfig {
      phi: 10.0,
      capacity: 10,
      times: 3,
      req_tries: 3,
      req_timeout: Duration::from_millis(100),
    }
  }
}

#[derive(AurumInterface, Serialize, Deserialize)]
pub enum HeartbeatReceiverMsg {
  Heartbeat(Duration, u32),
}

pub(crate) enum HBRState {
  Initial(usize),
  Receiving(IntervalStorage, Duration, u32),
}

pub(crate) struct HeartbeatReceiver<U>
where
  U: UnifiedBounds + Case<HeartbeatReceiverMsg>,
{
  supervisor: LocalRef<ClusterMsg<U>>,
  member: Arc<Member>,
  charge: Arc<Member>,
  state: HBRState,
  config: HBRConfig
}
impl<U> HeartbeatReceiver<U>
where
  U: UnifiedBounds + Case<HeartbeatReceiverMsg>,
{
  pub fn spawn(
    ctx: &ActorContext<U, ClusterMsg<U>>,
    common: &NodeState<U>,
    charge: Arc<Member>,
  ) -> LocalRef<HeartbeatReceiverMsg> {
    ctx
      .node
      .spawn_timeout(
        HeartbeatReceiver {
          supervisor: ctx.local_interface(),
          member: common.member.clone(),
          charge: charge,
          state: HBRState::Initial(common.hbr_config.req_tries),
          config: common.hbr_config.clone()
        },
        format!("{}-{}", common.dest.name.name, common.member.id),
        true,
        common.hbr_config.req_timeout,
      )
      .local()
      .clone()
      .unwrap()
  }
}
#[async_trait]
impl<U> TimeoutActor<U, HeartbeatReceiverMsg> for HeartbeatReceiver<U>
where
  U: UnifiedBounds + Case<HeartbeatReceiverMsg>,
{
  async fn recv(
    &mut self,
    _: &ActorContext<U, HeartbeatReceiverMsg>,
    msg: HeartbeatReceiverMsg,
  ) -> Option<Duration> {
    let state: Option<HBRState> = match &mut self.state {
      HBRState::Initial(_) => match msg {
        Heartbeat(dur, cnt) => {
          let is = IntervalStorage::new(self.config.capacity, dur, self.config.times, None);
          Some(HBRState::Receiving(is, dur, cnt))
        }
      },
      HBRState::Receiving(_, _, _) => None,
    };
    state.into_iter().for_each(|s| self.state = s);
    None
  }

  async fn timeout(
    &mut self,
    _: &crate::core::ActorContext<U, HeartbeatReceiverMsg>,
  ) -> Option<Duration> {
    None
  }
}
