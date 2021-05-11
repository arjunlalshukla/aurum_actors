#![allow(dead_code, unused_imports)]
use crate as aurum;
use crate::cluster::{ClusterCmd, ClusterUpdate, Member, NodeRing};
use crate::cluster::crdt::{CausalCmd, CausalDisperse, DispersalPreference};
use crate::cluster::devices::{Device, Devices, DeviceInterval, HBReqSenderMsg};
use crate::core::{Actor, ActorContext, Case, LocalRef, Node, Socket, UnifiedType};
use crate::testkit::{FailureConfigMap};
use crate::AurumInterface;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::marker::PhantomData;
use std::sync::Arc;

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
  servers: im::HashSet<Arc<Member>>,
  ring: NodeRing,
  devices: Devices,
  req_senders: HashMap<Device, LocalRef<HBReqSenderMsg>>,
  charges: im::HashSet<Device>,
  x: PhantomData<U>
}
impl<U: UnifiedType> TryFrom<Waiting> for InCluster<U> {
  type Error = ();
  fn try_from(value: Waiting) -> Result<Self, Self::Error> {
    if value.servers.is_some() && value.ring.is_some() && value.devices.is_some() {
      Ok(Self {
        servers: value.servers.unwrap(),
        ring: value.ring.unwrap(),
        devices: value.devices.unwrap(),
        req_senders: HashMap::new(),
        charges: im::hashset![],
        x: PhantomData,
      })
    } else {
      Err(())
    }
  }
}


struct Waiting {
  servers: Option<im::HashSet<Arc<Member>>>,
  ring: Option<NodeRing>,
  devices: Option<Devices>,
}

enum State<U: UnifiedType> {
  Waiting(Waiting),
  InCluster(InCluster<U>)
}

struct Common {
  causal: LocalRef<CausalCmd<Devices>>,
  cluster: LocalRef<ClusterCmd>,
  subscribers: Vec<LocalRef<Charges>>,
}

pub struct DeviceServer<U: UnifiedType> {
  common: Common,
  state: State<U>
}
impl<U: UnifiedType> DeviceServer<U> {
  fn new(
    node: &Node<U>,
    cluster: LocalRef<ClusterCmd>,
    subscribers: Vec<LocalRef<Charges>>,
    name: String,
    fail_map: FailureConfigMap,
  ) -> LocalRef<DeviceServerCmd> {
    let causal = CausalDisperse::new(
      node, 
      format!("{}-devices", name),
      fail_map, 
      vec![],
      DispersalPreference::default(),
      cluster.clone()
    );
    let common = Common {
      causal: causal,
      cluster: cluster,
      subscribers: subscribers
    };
    let actor = Self {
      common: common,
      state: State::Waiting(Waiting {
        servers: None,
        ring: None,
        devices: None,
      })
    };
    node.spawn(false, actor, name, true).local().clone().unwrap().transform()
  }
}
#[async_trait]
impl<U: UnifiedType> Actor<U, DeviceServerMsg> for DeviceServer<U> {
  async fn recv(&mut self, ctx: &ActorContext<U, DeviceServerMsg>, msg: DeviceServerMsg) {
    todo!()
  }
}