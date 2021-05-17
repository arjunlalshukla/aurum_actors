use crate as aurum;
use crate::cluster::crdt::{CausalCmd, CausalDisperse, DispersalPreference};
use crate::cluster::devices::{
  Device, DeviceInterval, DeviceMutator, Devices, HBReqSender,
  HBReqSenderConfig, HBReqSenderMsg, LOG_LEVEL,
};
use crate::cluster::{
  ClusterCmd, ClusterEvent, ClusterUpdate, Member, NodeRing, FAILURE_MODE,
};
use crate::core::{
  Actor, ActorContext, ActorSignal, Destination, LocalRef, Node, UnifiedType,
};
use crate::testkit::FailureConfigMap;
use crate::{debug, trace, udp_select, AurumInterface};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use DeviceServerMsg::*;
use DeviceServerRemoteMsg::*;

#[derive(AurumInterface)]
#[aurum(local)]
pub enum DeviceServerMsg {
  #[aurum]
  Remote(DeviceServerRemoteMsg),
  #[aurum(local)]
  Update(ClusterUpdate),
  #[aurum(local)]
  DeviceData(Devices),
  #[aurum(local)]
  Cmd(DeviceServerCmd),
  AmISender(Device),
  DownedDevice(Device),
}

#[derive(Serialize, Deserialize)]
pub enum DeviceServerRemoteMsg {
  SetHeartbeatInterval(Device, DeviceInterval),
}

pub enum DeviceServerCmd {
  Subscribe(LocalRef<Charges>),
}

pub struct Charges(pub im::HashSet<Device>);

struct InCluster<U: UnifiedType> {
  member: Arc<Member>,
  nodes: im::HashSet<Arc<Member>>,
  ring: NodeRing,
  devices: Devices,
  req_senders: HashMap<Device, LocalRef<HBReqSenderMsg>>,
  charges: im::HashSet<Device>,
  x: PhantomData<U>,
}
impl<U: UnifiedType> InCluster<U> {
  fn remove_device(&mut self, common: &mut Common<U>, device: &Device) {
    if let Some(r) = self.req_senders.remove(&device) {
      r.signal(ActorSignal::Term);
      self.charges.remove(&device);
      common
        .subscribers
        .retain(|s| s.send(Charges(self.charges.clone())));
    }
  }

  fn is_manager(&self, device: &Device) -> bool {
    self
      .ring
      .managers(&device, 1)
      .iter()
      .any(|m| m.id == self.member.id)
  }
}

#[derive(Clone)]
struct Waiting {
  servers: Option<im::HashSet<Arc<Member>>>,
  member: Option<Arc<Member>>,
  ring: Option<NodeRing>,
  devices: Option<Devices>,
}
impl Waiting {
  fn is_ready(&self) -> bool {
    self.servers.is_some()
      && self.ring.is_some()
      && self.devices.is_some()
      && self.member.is_some()
  }
}

enum State<U: UnifiedType> {
  Waiting(Waiting),
  InCluster(InCluster<U>),
}
impl<U: UnifiedType> State<U> {
  fn to_ic(&mut self) {
    if let State::Waiting(w) = self {
      if w.is_ready() {
        let w = w.clone();
        *self = State::InCluster(InCluster {
          member: w.member.unwrap(),
          nodes: w.servers.unwrap(),
          ring: w.ring.unwrap(),
          devices: w.devices.unwrap(),
          req_senders: HashMap::new(),
          charges: im::hashset![],
          x: PhantomData,
        });
      }
    }
  }
}

struct Common<U: UnifiedType> {
  causal: LocalRef<CausalCmd<Devices>>,
  cluster: LocalRef<ClusterCmd>,
  subscribers: Vec<LocalRef<Charges>>,
  fail_map: FailureConfigMap,
  dest: Destination<U, DeviceServerRemoteMsg>,
}

pub struct DeviceServer<U: UnifiedType> {
  common: Common<U>,
  state: State<U>,
}
impl<U: UnifiedType> DeviceServer<U> {
  pub fn new(
    node: &Node<U>,
    cluster: LocalRef<ClusterCmd>,
    subscribers: Vec<LocalRef<Charges>>,
    name: String,
    fail_map: FailureConfigMap,
  ) -> LocalRef<DeviceServerCmd> {
    let causal = CausalDisperse::new(
      node,
      format!("{}-devices", name),
      fail_map.clone(),
      vec![],
      DispersalPreference::default(),
      cluster.clone(),
    );
    let common = Common {
      causal: causal,
      cluster: cluster,
      fail_map: fail_map,
      subscribers: subscribers,
      dest: Destination::new::<DeviceServerMsg>(name.clone()),
    };
    let actor = Self {
      common: common,
      state: State::Waiting(Waiting {
        member: None,
        servers: None,
        ring: None,
        devices: None,
      }),
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
impl<U: UnifiedType> Actor<U, DeviceServerMsg> for DeviceServer<U> {
  async fn pre_start(&mut self, ctx: &ActorContext<U, DeviceServerMsg>) {
    let msg = ClusterCmd::Subscribe(ctx.local_interface());
    self.common.cluster.send(msg);
    let msg = CausalCmd::Subscribe(ctx.local_interface());
    self.common.causal.send(msg);
  }

  async fn recv(
    &mut self,
    ctx: &ActorContext<U, DeviceServerMsg>,
    msg: DeviceServerMsg,
  ) {
    match msg {
      Remote(SetHeartbeatInterval(device, interval)) => {
        if let State::InCluster(ic) = &mut self.state {
          let hbr_sender = ic.req_senders.get(&device);
          let manager =
            ic.ring.managers(&device, 1).into_iter().next().unwrap();
          if (hbr_sender.is_some() || manager == ic.member) {
            self
              .common
              .causal
              .send(CausalCmd::Mutate(DeviceMutator::Put(
                device.clone(),
                interval.clone(),
              )));
            let msg = HBReqSenderMsg::Interval(interval);
            if let Some(sender) = hbr_sender {
              sender.send(msg);
            } else {
              let sender = HBReqSender::new(
                &ctx.node,
                ctx.local_interface(),
                HBReqSenderConfig::default(),
                device.clone(),
                interval,
                self.common.dest.name().name.clone(),
              );
              sender.send(msg);
              ic.req_senders.insert(device.clone(), sender);
              ic.charges.insert(device);
              self
                .common
                .subscribers
                .retain(|s| s.send(Charges(ic.charges.clone())));
            }
          } else {
            udp_select!(
              FAILURE_MODE,
              &ctx.node,
              &self.common.fail_map,
              &manager.socket,
              &self.common.dest,
              &SetHeartbeatInterval(device, interval)
            );
          }
        } else if let State::Waiting(w) = &mut self.state {
          debug!(
            LOG_LEVEL,
            &ctx.node,
            format!(
              "Waiting for {}, {}, {}, {}, but got HB interval from {}, {:?}",
              w.member.is_some(),
              w.ring.is_some(),
              w.servers.is_some(),
              w.devices.is_some(),
              device.socket.udp,
              interval
            )
          );
        }
      }
      Update(update) => {
        let member = update
          .events
          .into_iter()
          .next()
          .map(|e| match e {
            ClusterEvent::Joined(m) => Some(m),
            ClusterEvent::Alone(m) => Some(m),
            _ => None,
          })
          .flatten();
        match &mut self.state {
          State::InCluster(ic) => {
            if let Some(m) = member {
              ic.member = m;
            }
            ic.ring = update.ring;
            ic.nodes = update.nodes;
          }
          State::Waiting(w) => {
            if let Some(member) = member {
              w.member.replace(member);
            }
            w.ring = Some(update.ring);
            w.servers = Some(update.nodes);
            self.state.to_ic();
          }
        }
      }
      DeviceData(devices) => match &mut self.state {
        State::Waiting(w) => {
          w.devices = Some(devices);
          self.state.to_ic();
        }
        State::InCluster(ic) => ic.devices = devices,
      },
      Cmd(cmd) => match cmd {
        DeviceServerCmd::Subscribe(subr) => {
          if let State::InCluster(ic) = &mut self.state {
            subr.send(Charges(ic.charges.clone()));
          }
          self.common.subscribers.push(subr);
        }
      },
      AmISender(device) => {
        if let State::InCluster(ic) = &mut self.state {
          if ic.is_manager(&device) {
            trace!(
              LOG_LEVEL,
              &ctx.node,
              format!("Keeping sender for {:?}", device)
            );
          } else {
            trace!(
              LOG_LEVEL,
              &ctx.node,
              format!("Killing not-sender for {:?}", device)
            );
            ic.remove_device(&mut self.common, &device);
          }
        } else {
          unreachable!()
        }
      }
      DownedDevice(device) => {
        if let State::InCluster(ic) = &mut self.state {
          ic.remove_device(&mut self.common, &device);
          self
            .common
            .causal
            .send(CausalCmd::Mutate(DeviceMutator::Remove(device)));
        } else {
          unreachable!()
        }
      }
    }
  }

  async fn post_stop(&mut self, _: &ActorContext<U, DeviceServerMsg>) {
    if let State::InCluster(ic) = &self.state {
      for sender in ic.req_senders.values() {
        sender.signal(ActorSignal::Term);
      }
    }
  }
}
