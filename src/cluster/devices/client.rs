use crate as aurum;
use crate::cluster::devices::{
  DeviceInterval, DeviceServerRemoteMsg, HBReqSenderRemoteMsg,
};
use crate::core::{Actor, ActorContext, ActorRef, Socket, UnifiedType};
use crate::AurumInterface;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use DeviceClientCmd::*;
use DeviceClientMsg::*;
use DeviceClientRemoteMsg::*;

#[derive(Clone, Serialize, Deserialize)]
pub struct DeviceClientConfig {
  pub phi: f64,
  pub storage_capacity: usize,
  pub times: usize,
  pub log_capacity: usize,
  pub seeds: Vec<Socket>,
}
impl Default for DeviceClientConfig {
  fn default() -> Self {
    Self {
      phi: 0.995,
      storage_capacity: 10,
      times: 5,
      log_capacity: 10,
      seeds: vec![],
    }
  }
}

#[derive(AurumInterface)]
#[aurum(local)]
pub enum DeviceClientMsg<U: UnifiedType> {
  Tick,
  #[aurum]
  Remote(DeviceClientRemoteMsg<U>),
  Cmd(DeviceClientCmd),
}

#[derive(Serialize, Deserialize)]
#[serde(bound = "U: UnifiedType")]
pub enum DeviceClientRemoteMsg<U: UnifiedType> {
  HeartbeatRequest(ActorRef<U, HBReqSenderRemoteMsg>),
  IntervalAck(Socket, u64),
}

pub enum DeviceClientCmd {
  SetInterval(Duration),
}

struct DeviceClient {}
impl DeviceClient {}
#[async_trait]
impl<U: UnifiedType> Actor<U, DeviceClientMsg<U>> for DeviceClient {
  async fn recv(
    &mut self,
    ctx: &ActorContext<U, DeviceClientMsg<U>>,
    msg: DeviceClientMsg<U>,
  ) {
    match msg {
      Tick => {}
      Remote(HeartbeatRequest(sender)) => {}
      Remote(IntervalAck(socket, clock)) => {}
      Cmd(SetInterval(dur)) => {}
    }
  }
}
