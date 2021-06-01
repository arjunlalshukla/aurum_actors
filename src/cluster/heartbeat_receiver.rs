use crate as aurum;
use crate::cluster::{
  ClusterMsg, HBRConfig, IntervalStorage, IntraClusterMsg, Member, NodeState,
  FAILURE_MODE, LOG_LEVEL,
};
use crate::core::{
  ActorContext, Destination, LocalRef, TimeoutActor, UnifiedType,
};
use crate::testkit::FailureConfigMap;
use crate::{debug, info, trace, warn, udp_select, AurumInterface};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use HeartbeatReceiverMsg::*;

#[derive(AurumInterface, Serialize, Deserialize)]
pub enum HeartbeatReceiverMsg {
  Heartbeat(Duration, u32),
}

pub(crate) enum HBRState {
  Initial(usize),
  Receiving(IntervalStorage, u32),
  Downed,
}

pub(crate) struct HeartbeatReceiver<U: UnifiedType> {
  supervisor: LocalRef<ClusterMsg<U>>,
  fail_map: FailureConfigMap,
  clr_dest: Destination<U, IntraClusterMsg<U>>,
  charge: Arc<Member>,
  req: IntraClusterMsg<U>,
  state: HBRState,
  config: HBRConfig,
}
impl<U: UnifiedType> HeartbeatReceiver<U> {
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
          fail_map: common.fail_map.clone(),
          charge: charge,
          clr_dest: common.clr_dest.clone(),
          req: IntraClusterMsg::ReqHeartbeat(common.member.clone(), cid),
          state: HBRState::Initial(common.hbr_config.req_tries),
          config: common.hbr_config.clone(),
        },
        Self::from_clr(common.clr_dest.name().name.as_str(), cid),
        true,
        common.hbr_config.req_timeout,
      )
      .local()
      .clone()
      .unwrap()
  }

  async fn send_req(&self, ctx: &ActorContext<U, HeartbeatReceiverMsg>) {
    udp_select!(
      FAILURE_MODE,
      &ctx.node,
      &self.fail_map,
      &self.charge.socket,
      &self.clr_dest,
      &self.req
    );
  }
}
#[async_trait]
impl<U> TimeoutActor<U, HeartbeatReceiverMsg> for HeartbeatReceiver<U>
where
  U: UnifiedType,
{
  async fn pre_start(
    &mut self,
    ctx: &ActorContext<U, HeartbeatReceiverMsg>,
  ) -> Option<Duration> {
    debug!(
      LOG_LEVEL,
      ctx.node,
      format!(
        "killed HBR for {}-{}",
        self.charge.socket.udp, self.charge.id
      )
    );
    self.send_req(ctx).await;
    None
  }

  async fn recv(
    &mut self,
    ctx: &ActorContext<U, HeartbeatReceiverMsg>,
    msg: HeartbeatReceiverMsg,
  ) -> Option<Duration> {
    let state = match &mut self.state {
      HBRState::Initial(_) => match msg {
        Heartbeat(dur, cnt) => {
          debug!(
            LOG_LEVEL,
            ctx.node,
            format!(
              "new heartbeat interval from {}: {:?} ms",
              self.charge.socket.udp,
              dur.as_millis()
            )
          );
          let is = IntervalStorage::new(
            self.config.capacity,
            dur * 2,
            self.config.times,
            None,
          );
          let new_dur = Some(is.duration_phi(self.config.phi));
          let new_state = Some(HBRState::Receiving(is, cnt));
          (new_dur, new_state)
        }
      },
      HBRState::Receiving(storage, cnt) => match msg {
        Heartbeat(new_dur, new_cnt) => {
          if new_cnt > *cnt {
            info!(
              LOG_LEVEL,
              ctx.node,
              format!(
                "new heartbeat interval from {}: {:?} ms",
                self.charge.socket.udp,
                new_dur.as_millis()
              )
            );
            *storage = IntervalStorage::new(
              self.config.capacity,
              new_dur * 2,
              self.config.times,
              None,
            );
          } else {
            storage.push();
          }
          let new_to = storage.duration_phi(self.config.phi);
          trace!(
            LOG_LEVEL,
            ctx.node,
            format!(
              "got heartbeat from {}; new timeout: {:?} ms, stdev: {}, mean: {}",
              self.charge.socket.udp,
              new_to.as_millis(),
              storage.stdev(),
              storage.mean()
            )
          );
          (Some(new_to), None)
        }
      },
      HBRState::Downed => (Some(Duration::from_secs(u32::MAX as u64)), None),
    };
    state.1.into_iter().for_each(|s| self.state = s);
    state.0
  }

  async fn timeout(
    &mut self,
    ctx: &ActorContext<U, HeartbeatReceiverMsg>,
  ) -> Option<Duration> {
    let state = match &mut self.state {
      HBRState::Initial(0) => {
        warn!(
          LOG_LEVEL,
          ctx.node,
          format!(
            "DOWNED charge {}; after timeout: {} ms",
            self.charge.socket,
            self.config.req_timeout.as_millis()
          )
        );
        self
          .supervisor
          .send(ClusterMsg::Downed(self.charge.clone()));
        (
          Some(Duration::from_secs(u32::MAX as u64)),
          Some(HBRState::Downed),
        )
      }
      HBRState::Receiving(storage, _) => {
        debug!(
          LOG_LEVEL,
          ctx.node,
          format!(
            "requesting HB from {}; after timeout: {} ms, stdev: {}, mean: {}",
            self.charge.socket.udp,
            storage.duration_phi(self.config.phi).as_millis(),
            storage.stdev(),
            storage.mean()
          )
        );
        (
          Some(self.config.req_timeout),
          Some(HBRState::Initial(self.config.req_tries)),
        )
      }
      HBRState::Initial(ref mut reqs_left) => {
        *reqs_left -= 1;
        self.send_req(ctx).await;
        (Some(self.config.req_timeout), None)
      }
      HBRState::Downed => (Some(Duration::from_secs(u32::MAX as u64)), None),
    };
    state.1.into_iter().for_each(|s| self.state = s);
    state.0
  }

  async fn post_stop(
    &mut self,
    ctx: &ActorContext<U, HeartbeatReceiverMsg>,
  ) -> Option<Duration> {
    debug!(
      LOG_LEVEL,
      ctx.node,
      format!(
        "killed HBR for {}-{}",
        self.charge.socket.udp, self.charge.id
      )
    );
    None
  }
}
