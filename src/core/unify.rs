use crate::cluster::{ClusterMsg, HeartbeatReceiverMsg, IntraClusterMsg};
use crate::cluster::devices::{Devices, DeviceServerMsg, DeviceServerRemoteMsg};
use crate::cluster::crdt::{CausalIntraMsg, CausalMsg};
use crate::core::{
  DeserializeError, Interpretations, LocalActorMsg, RegistryMsg,
};
use crate::testkit::LoggerMsg;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::{Debug, Display};
use std::hash::Hash;

pub trait UnifiedType:
  'static
  + Send
  + Sync
  + Copy
  + Clone
  + PartialEq
  + Eq
  + Hash
  + Debug
  + Display
  + Serialize
  + DeserializeOwned
  + PartialOrd
  + Ord
  + Case<RegistryMsg<Self>>
  + Case<LoggerMsg>
  + Case<ClusterMsg<Self>>
  + Case<IntraClusterMsg<Self>>
  + Case<HeartbeatReceiverMsg>
  + Case<DeviceServerMsg>
  + Case<DeviceServerRemoteMsg>
  + Case<CausalMsg<Devices>>
  + Case<CausalIntraMsg<Devices>>
{
  fn has_interface(self, interface: Self) -> bool;
}

pub trait Case<S> {
  const VARIANT: Self;
}

pub trait SpecificInterface<U: UnifiedType + Case<Self>>
where
  Self: Sized + Send + 'static ,
{
  fn deserialize_as(
    interface: U,
    intp: Interpretations,
    bytes: &[u8],
  ) -> Result<LocalActorMsg<Self>, DeserializeError<U>>;

  fn has_interface(interface: U) -> bool;
}
