use crate::cluster::devices::{
  Device, DeviceInterval, DeviceServerMsg, DeviceServerRemoteMsg,
  HBReqSenderRemoteMsg, LOG_LEVEL,
};
use crate::cluster::{IntervalStorage, FAILURE_MODE};
use crate::core::{
  Actor, ActorContext, ActorRef, LocalRef, Node, Socket, UnifiedType,
};
use crate::testkit::FailureConfigMap;
use crate::{self as aurum, core::Destination};
use crate::{debug, trace, udp_select, AurumInterface};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{hash_map::Entry::*, HashMap, VecDeque};
use std::hash::Hash;
use std::time::Duration;
use DeviceClientCmd::*;
use DeviceClientMsg::*;
use DeviceClientRemoteMsg::*;
use DeviceServerRemoteMsg::*;
use HBReqSenderRemoteMsg::*;

#[derive(Clone, Serialize, Deserialize)]
pub struct DeviceClientConfig {
  pub phi: f64,
  pub storage_capacity: u32,
  pub times: u32,
  pub log_capacity: u32,
  pub seeds: im::HashSet<Socket>,
}
impl Default for DeviceClientConfig {
  fn default() -> Self {
    Self {
      phi: 0.995,
      storage_capacity: 10,
      times: 5,
      log_capacity: 10,
      seeds: im::hashset![],
    }
  }
}
impl DeviceClientConfig {
  fn new_storage(&self, init: Duration) -> IntervalStorage {
    IntervalStorage::new(
      self.storage_capacity as usize,
      init,
      self.times as usize,
      None,
    )
  }
}

#[derive(AurumInterface)]
#[aurum(local)]
pub enum DeviceClientMsg<U: UnifiedType> {
  Tick,
  #[aurum]
  Remote(DeviceClientRemoteMsg<U>),
  #[aurum(local)]
  Cmd(DeviceClientCmd),
}

#[derive(Serialize, Deserialize)]
#[serde(bound = "U: UnifiedType")]
pub enum DeviceClientRemoteMsg<U: UnifiedType> {
  HeartbeatRequest(ActorRef<U, HBReqSenderRemoteMsg>),
  IntervalAck(Socket, DeviceInterval),
}

pub enum DeviceClientCmd {
  SetInterval(Duration),
  //Subscribe(LocalRef<DeviceClientCmd>)
}

pub struct DeviceClient<U: UnifiedType> {
  my_info: Device,
  interval: DeviceInterval,
  config: DeviceClientConfig,
  svr_dest: Destination<U, DeviceServerRemoteMsg>,
  server: Option<Socket>,
  server_pov: DeviceInterval,
  storage: IntervalStorage,
  server_log: FrequencyBuffer<ActorRef<U, HBReqSenderRemoteMsg>>,
  fail_map: FailureConfigMap,
}
impl<U: UnifiedType> DeviceClient<U> {
  pub fn new(
    node: &Node<U>,
    initial_interval: Duration,
    config: DeviceClientConfig,
    name: String,
    fail_map: FailureConfigMap,
  ) -> LocalRef<DeviceClientCmd> {
    let actor = Self {
      my_info: Device {
        socket: node.socket().clone(),
      },
      interval: DeviceInterval {
        clock: 1,
        interval: initial_interval,
      },
      server: None,
      server_pov: DeviceInterval {
        clock: 0,
        interval: initial_interval,
      },
      svr_dest: Destination::new::<DeviceServerMsg>(name.clone()),
      storage: config.new_storage(initial_interval),
      server_log: FrequencyBuffer::new(config.log_capacity),
      config: config,
      fail_map: fail_map,
    };
    node
      .spawn(false, actor, name, true)
      .local()
      .clone()
      .unwrap()
      .transform()
  }

  async fn notify_server(&self, ctx: &ActorContext<U, DeviceClientMsg<U>>) {
    let msg = SetHeartbeatInterval(self.my_info.clone(), self.interval);
    match &self.server {
      Some(svr) => {
        trace!(
          LOG_LEVEL,
          &ctx.node,
          format!("On server: SETTING {} to {:?}", svr.udp, self.interval)
        );
        udp_select!(
          FAILURE_MODE,
          &ctx.node,
          &self.fail_map,
          svr,
          &self.svr_dest,
          &msg
        );
      }
      None => {
        trace!(
          LOG_LEVEL,
          &ctx.node,
          format!(
            "Server undefined: on {:?}, SETTING to {:?}",
            self.config.seeds.iter().map(|x| x.udp).collect::<Vec<_>>(),
            self.interval
          )
        );
        for seed in self.config.seeds.iter() {
          udp_select!(
            FAILURE_MODE,
            &ctx.node,
            &self.fail_map,
            seed,
            &self.svr_dest,
            &msg
          );
        }
      }
    }
  }

  async fn new_interval(
    &mut self,
    dur: Duration,
    ctx: &ActorContext<U, DeviceClientMsg<U>>,
  ) {
    self.interval.clock += 1;
    self.interval.interval = dur;
    self.notify_server(ctx).await;
  }
}
#[async_trait]
impl<U: UnifiedType> Actor<U, DeviceClientMsg<U>> for DeviceClient<U> {
  async fn pre_start(&mut self, ctx: &ActorContext<U, DeviceClientMsg<U>>) {
    println!("My server actor name: {:?}", self.svr_dest.name());
    self.notify_server(ctx).await;
    ctx.node.schedule_local_msg(
      self.interval.interval,
      ctx.local_interface(),
      Tick,
    );
  }

  async fn recv(
    &mut self,
    ctx: &ActorContext<U, DeviceClientMsg<U>>,
    msg: DeviceClientMsg<U>,
  ) {
    match msg {
      Tick => {
        trace!(LOG_LEVEL, &ctx.node, "Received tick");
        if self.storage.phi() > self.config.phi {
          debug!(LOG_LEVEL, &ctx.node, "Assuming the server is down");
          self.server = None;
          self.notify_server(ctx).await;
        }
        ctx.node.schedule_local_msg(
          self.interval.interval,
          ctx.local_interface(),
          Tick,
        );
      }
      Remote(HeartbeatRequest(sender)) => {
        trace!(
          LOG_LEVEL,
          &ctx.node,
          format!("Heartbeat request from {} received:", sender.socket.udp)
        );
        if self
          .server
          .as_ref()
          .filter(|x| *x == &sender.socket)
          .is_none()
        {
          self.storage = self.config.new_storage(self.interval.interval);
          self.server = Some(sender.socket.clone());
          self.config.seeds.insert(sender.socket.clone());
        }
        self.server_log.push(sender.clone());
        if self.server_log.changes + 1
          == self.server_log.frequencies.len() as u32
        {
          debug!(LOG_LEVEL, &ctx.node, "Multiple senders detected");
          for svr in self.server_log.frequencies.keys() {
            svr.remote_send(&MultipleSenders).await;
          }
        }
      }
      Remote(IntervalAck(socket, interval)) => {
        trace!(
          LOG_LEVEL,
          &ctx.node,
          format!(
            "IntervalAck from {} received: {:?}; previous: {:?}; current: {:?}",
            socket.udp, interval, self.server_pov, self.interval
          )
        );
        self.server = Some(socket);
        self.server_pov = interval;
        if self.server_pov == self.interval {
          self.storage = self.config.new_storage(self.interval.interval);
        } else if self.server_pov.clock < self.interval.clock {
          self.notify_server(ctx).await;
        } else {
          self.interval.clock = self.server_pov.clock;
          self.new_interval(self.interval.interval, ctx).await;
        }
      }
      Cmd(SetInterval(dur)) => self.new_interval(dur, ctx).await,
    }
  }
}

struct FrequencyBuffer<T: Eq + PartialEq + Clone + Hash> {
  buffer: VecDeque<T>,
  frequencies: HashMap<T, u32>,
  capacity: u32,
  changes: u32,
}
impl<T: Eq + PartialEq + Clone + Hash> FrequencyBuffer<T> {
  fn new(cap: u32) -> Self {
    Self {
      buffer: VecDeque::new(),
      frequencies: HashMap::new(),
      capacity: cap,
      changes: 0,
    }
  }

  fn pop(&mut self) {
    if let Some(removed) = self.buffer.pop_back() {
      if self.buffer.back().filter(|x| *x == &removed).is_none() {
        self.changes -= 1;
      }
      if let Occupied(mut o) = self.frequencies.entry(removed) {
        let m = o.get_mut();
        if *m == 1 {
          o.remove();
        } else {
          *m -= 1
        }
      }
    }
  }

  fn push(&mut self, item: T) {
    while self.buffer.len() >= self.capacity as usize {
      self.pop();
    }
    if self.buffer.front().filter(|x| *x == &item).is_none() {
      self.changes += 1;
    }
    match self.frequencies.entry(item.clone()) {
      Occupied(mut o) => {
        let m = o.get_mut();
        *m += 1
      }
      Vacant(v) => {
        v.insert(1);
      }
    }
    self.buffer.push_front(item);
  }
}
