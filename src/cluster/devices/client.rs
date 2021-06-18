use crate::cluster::devices::{
  Device, DeviceInterval, DeviceServerMsg, DeviceServerRemoteMsg, HBReqSenderRemoteMsg, LOG_LEVEL,
};
use crate::cluster::{IntervalStorage, FAILURE_MODE};
use crate::core::{Actor, ActorContext, ActorRef, LocalRef, Node, Socket, UdpSerial, UnifiedType};
use crate::testkit::FailureConfigMap;
use crate::{self as aurum, core::Destination};
use crate::{info, trace, AurumInterface};
use async_trait::async_trait;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::{hash_map::Entry::*, HashMap, VecDeque};
use std::hash::Hash;
use std::sync::Arc;
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
  pub initial_interval: Duration,
}
impl Default for DeviceClientConfig {
  fn default() -> Self {
    Self {
      phi: 0.995,
      storage_capacity: 10,
      times: 5,
      log_capacity: 10,
      seeds: im::hashset![],
      initial_interval: Duration::from_millis(1000),
    }
  }
}
impl DeviceClientConfig {
  fn new_storage(&self, init: Duration) -> IntervalStorage {
    IntervalStorage::new(self.storage_capacity as usize, init * 2, self.times as usize, None)
  }
}

pub struct Manager(pub Option<Socket>);

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
  Subscribe(LocalRef<Manager>),
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
  subscribers: Vec<LocalRef<Manager>>,
}
impl<U: UnifiedType> DeviceClient<U> {
  pub fn new(
    node: &Node<U>,
    config: DeviceClientConfig,
    name: String,
    fail_map: FailureConfigMap,
    subscribers: Vec<LocalRef<Manager>>,
  ) -> LocalRef<DeviceClientCmd> {
    let actor = Self {
      my_info: Device {
        socket: node.socket().clone(),
      },
      interval: DeviceInterval {
        clock: 1,
        interval: config.initial_interval,
      },
      server: None,
      server_pov: DeviceInterval {
        clock: 0,
        interval: config.initial_interval,
      },
      svr_dest: Destination::new::<DeviceServerMsg>(name.clone()),
      storage: config.new_storage(config.initial_interval),
      server_log: FrequencyBuffer::new(config.log_capacity),
      config: config,
      fail_map: fail_map,
      subscribers: subscribers,
    };
    node.spawn(false, actor, name, true).local().clone().unwrap().transform()
  }

  async fn notify_server(&self, ctx: &ActorContext<U, DeviceClientMsg<U>>) {
    let msg = SetHeartbeatInterval(self.my_info.clone(), self.interval);
    let ser = Arc::new(UdpSerial::msg(&self.svr_dest, &msg));
    match &self.server {
      Some(svr) => {
        trace!(
          LOG_LEVEL,
          &ctx.node,
          format!("On server: SETTING {} to {:?}", svr.udp, self.interval)
        );
        ctx.node.udp_select(svr, &ser, FAILURE_MODE, &self.fail_map).await;
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
          ctx.node.udp_select(seed, &ser, FAILURE_MODE, &self.fail_map).await;
        }
      }
    }
  }

  async fn new_interval(&mut self, dur: Duration, ctx: &ActorContext<U, DeviceClientMsg<U>>) {
    self.interval.clock += 1;
    self.interval.interval = dur;
    self.notify_server(ctx).await;
  }

  fn set_server(&mut self, svr: Option<Socket>) {
    self.subscribers.retain(|s| s.send(Manager(svr.clone())));
    self.server = svr;
  }
}
#[async_trait]
impl<U: UnifiedType> Actor<U, DeviceClientMsg<U>> for DeviceClient<U> {
  async fn pre_start(&mut self, ctx: &ActorContext<U, DeviceClientMsg<U>>) {
    self.notify_server(ctx).await;
    ctx.node.schedule_local_msg(self.interval.interval, ctx.local_interface(), Tick);
  }

  async fn recv(&mut self, ctx: &ActorContext<U, DeviceClientMsg<U>>, msg: DeviceClientMsg<U>) {
    match msg {
      Tick => {
        let phi = self.storage.phi();
        trace!(LOG_LEVEL, &ctx.node, format!("Received tick; {:?}", self.storage));
        if phi > self.config.phi {
          info!(LOG_LEVEL, &ctx.node, "Assuming the server is down");
          self.set_server(None);
          self.notify_server(ctx).await;
        }
        ctx.node.schedule_local_msg(self.interval.interval, ctx.local_interface(), Tick);
      }
      Remote(HeartbeatRequest(sender)) => {
        trace!(
          LOG_LEVEL,
          &ctx.node,
          format!("Heartbeat request from {} received:", sender.socket.udp)
        );
        if self.server.as_ref().filter(|x| *x == &sender.socket).is_none() {
          self.storage = self.config.new_storage(self.interval.interval);
          self.set_server(Some(sender.socket.clone()));
          self.config.seeds.insert(sender.socket.clone());
        } else {
          self.storage.push();
        }
        let ser = Arc::new(UdpSerial::msg(&sender.dest, &Heartbeat));
        ctx.node.udp_select(&sender.socket, &ser, FAILURE_MODE, &self.fail_map).await;
        self.server_log.push(sender);
        if self.server_log.changes + 1 != self.server_log.frequencies.len() as u32 {
          let log = format!(
            "Multiple senders detected: {:?}",
            self.server_log.frequencies.keys().map(|x| x.socket.to_string()).collect_vec()
          );
          info!(LOG_LEVEL, &ctx.node, log);
          for svr in self.server_log.frequencies.keys() {
            let ser = Arc::new(UdpSerial::msg(&svr.dest, &MultipleSenders));
            ctx.node.udp_select(&svr.socket, &ser, FAILURE_MODE, &self.fail_map).await;
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
        self.set_server(Some(socket));
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
      Cmd(Subscribe(subr)) => {
        subr.send(Manager(self.server.clone()));
        self.subscribers.push(subr);
      }
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
      if self.buffer.back().filter(|x| *x != &removed).is_some() {
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
    if self.buffer.front().filter(|x| *x != &item).is_some() {
      self.changes += 1;
    }
    let count = self.frequencies.entry(item.clone()).or_insert(0);
    *count += 1;
    self.buffer.push_front(item);
  }
}
