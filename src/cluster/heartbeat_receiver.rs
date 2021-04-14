//#![allow(unused_imports, dead_code, unused_variables)]

use crate::{self as aurum, core::{ActorRef, Destination}};
use crate::cluster::{
  ClusterMsg, IntervalStorage, IntraClusterMsg, Member, NodeState, RELIABLE, UnifiedBounds,
};
use crate::core::{forge, ActorContext, Case, LocalRef, TimeoutActor};
use crate::{udp_send, AurumInterface};
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
  Receiving(IntervalStorage, u32),
  Downed
}

pub(crate) struct HeartbeatReceiver<U>
where
  U: UnifiedBounds + Case<HeartbeatReceiverMsg>,
{
  supervisor: LocalRef<ClusterMsg<U>>,
  member: Arc<Member>,
  clr_dest: Destination<U>,
  charge: Arc<Member>,
  req: IntraClusterMsg<U>,
  state: HBRState,
  config: HBRConfig
}
impl<U> HeartbeatReceiver<U>
where
  U: UnifiedBounds + Case<HeartbeatReceiverMsg>,
{
  pub fn from_clr(clr: &str, id: u64) -> String {
    format!("{}-{}", clr, id)
  }

  pub fn spawn(
    ctx: &ActorContext<U, ClusterMsg<U>>,
    common: &NodeState<U>,
    charge: Arc<Member>,
  ) -> LocalRef<HeartbeatReceiverMsg> {
    let cid = charge.id;
    ctx
      .node
      .spawn_timeout(
        HeartbeatReceiver {
          supervisor: ctx.local_interface(),
          member: common.member.clone(),
          charge: charge,
          clr_dest: common.clr_dest.clone(),
          req: IntraClusterMsg::ReqHeartbeat(common.member.clone(), cid),
          state: HBRState::Initial(common.hbr_config.req_tries),
          config: common.hbr_config.clone()
        },
        Self::from_clr(common.clr_dest.name.name.as_str(), common.member.id),
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
  async fn pre_start(&mut self, _: &ActorContext<U, HeartbeatReceiverMsg>) -> Option<Duration> {
    udp_send!(RELIABLE, &self.member.socket, &self.clr_dest, &self.req);
    None
  }

  async fn recv(
    &mut self,
    _: &ActorContext<U, HeartbeatReceiverMsg>,
    msg: HeartbeatReceiverMsg,
  ) -> Option<Duration> {
    let mut new_to = None;
    let state: Option<HBRState> = match &mut self.state {
      HBRState::Initial(_) => match msg {
        Heartbeat(dur, cnt) => {
          let is = IntervalStorage::new(self.config.capacity, dur, self.config.times, None);
          //new_to = Some(Duration::from_secs_f64((is.mean() + is.stdev()*2.0)*1000.0));
          Some(HBRState::Receiving(is, cnt))
        }
      },
      HBRState::Receiving(storage, cnt) => match msg {
        Heartbeat(new_dur, new_cnt) => {
          if new_cnt > *cnt {
            *storage = IntervalStorage::new(self.config.capacity, new_dur, self.config.times, None);
          } else {
            storage.push();
          }
          //new_to = Some(Duration::from_secs_f64((storage.mean() + storage.stdev()*2.0)*1000.0));
          None
        }
      },
      HBRState::Downed => None
    };
    state.into_iter().for_each(|s| self.state = s);
    new_to
  }

  async fn timeout(
    &mut self,
    _: &crate::core::ActorContext<U, HeartbeatReceiverMsg>,
  ) -> Option<Duration> {
    None
  }
}
