use crate as aurum;
use crate::cluster::devices::{
  Device, DeviceClientMsg, DeviceClientRemoteMsg, DeviceInterval,
  DeviceServerMsg, LOG_LEVEL,
};
use crate::cluster::{IntervalStorage, FAILURE_MODE};
use crate::core::{
  Actor, ActorContext, Destination, LocalRef, Node, UnifiedType,
};
use crate::testkit::FailureConfigMap;
use crate::{debug, trace, udp_select, AurumInterface};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use HBReqSenderMsg::*;
use HBReqSenderRemoteMsg::*;

#[derive(Clone, Serialize, Deserialize)]
pub struct HBReqSenderConfig {
  pub phi: f64,
  pub capacity: usize,
  pub times: usize,
}
impl Default for HBReqSenderConfig {
  fn default() -> Self {
    HBReqSenderConfig {
      phi: 0.995,
      capacity: 10,
      times: 5,
    }
  }
}

#[derive(AurumInterface, Serialize, Deserialize)]
pub enum HBReqSenderMsg {
  #[aurum]
  Remote(HBReqSenderRemoteMsg),
  Interval(DeviceInterval),
  Tick,
}

#[derive(Serialize, Deserialize)]
pub enum HBReqSenderRemoteMsg {
  Heartbeat,
  MultipleSenders,
}

pub struct HBReqSender<U: UnifiedType> {
  config: HBReqSenderConfig,
  storage: IntervalStorage,
  supervisor: LocalRef<DeviceServerMsg>,
  charge: Device,
  interval: DeviceInterval,
  fail_map: FailureConfigMap,
  dev_dest: Destination<U, DeviceClientRemoteMsg<U>>,
}
impl<U: UnifiedType> HBReqSender<U> {
  pub fn new(
    node: &Node<U>,
    supervisor: LocalRef<DeviceServerMsg>,
    config: HBReqSenderConfig,
    charge: Device,
    interval: DeviceInterval,
    name: String,
  ) -> LocalRef<HBReqSenderMsg> {
    let storage = IntervalStorage::new(
      config.capacity,
      interval.interval,
      config.times,
      None,
    );
    let id = rand::random::<u64>();
    let actor = Self {
      supervisor: supervisor,
      charge: charge,
      dev_dest: Destination::new::<DeviceClientMsg<U>>(name.clone()),
      interval: interval,
      config: config,
      storage: storage,
      fail_map: FailureConfigMap::default(),
    };
    node
      .spawn(false, actor, format!("{}-{}", name, id), true)
      .local()
      .clone()
      .unwrap()
  }
}
#[async_trait]
impl<U: UnifiedType> Actor<U, HBReqSenderMsg> for HBReqSender<U> {
  async fn pre_start(&mut self, ctx: &ActorContext<U, HBReqSenderMsg>) {
    ctx.local_interface().send(Tick);
  }

  async fn recv(
    &mut self,
    ctx: &ActorContext<U, HBReqSenderMsg>,
    msg: HBReqSenderMsg,
  ) {
    match msg {
      Tick => {
        debug!(
          LOG_LEVEL,
          &ctx.node,
          format!("Sending HBR to {:?}", self.charge)
        );
        if self.storage.phi() < self.config.phi {
          udp_select!(
            FAILURE_MODE,
            &ctx.node,
            &self.fail_map,
            &self.charge.socket,
            &self.dev_dest,
            &DeviceClientRemoteMsg::HeartbeatRequest(ctx.interface())
          );
          ctx.node.schedule_local_msg(
            self.interval.interval,
            ctx.local_interface(),
            Tick,
          );
        } else {
          self
            .supervisor
            .send(DeviceServerMsg::DownedDevice(self.charge.clone()));
        }
      }
      Interval(interval) => {
        debug!(
          LOG_LEVEL,
          &ctx.node,
          format!(
            "Got new interval for {}: {:?}",
            self.charge.socket.udp, interval
          )
        );
      }
      Remote(Heartbeat) => {
        trace!(
          LOG_LEVEL,
          &ctx.node,
          format!("Heartbeat from {}", self.charge.socket.udp)
        );
        self.storage.push();
      }
      Remote(MultipleSenders) => {
        self
          .supervisor
          .send(DeviceServerMsg::AmISender(self.charge.clone()));
      }
    }
  }
}
