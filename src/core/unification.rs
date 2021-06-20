use crate::cluster::crdt::{CausalIntraMsg, CausalMsg};
use crate::cluster::devices::{
  DeviceClientMsg, DeviceClientRemoteMsg, DeviceServerMsg, DeviceServerRemoteMsg, Devices,
  HBReqSenderMsg, HBReqSenderRemoteMsg,
};
use crate::cluster::{ClusterMsg, HeartbeatReceiverMsg, IntraClusterMsg};
use crate::core::{DeserializeError, Interpretations, LocalActorMsg, RegistryMsg};
use crate::testkit::LoggerMsg;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::{Debug, Display};
use std::hash::Hash;

/// Enumerates all possible message types within an application.
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
  + Case<DeviceClientMsg<Self>>
  + Case<DeviceClientRemoteMsg<Self>>
  + Case<DeviceServerMsg>
  + Case<DeviceServerRemoteMsg>
  + Case<HBReqSenderMsg>
  + Case<HBReqSenderRemoteMsg>
  + Case<CausalMsg<Devices>>
  + Case<CausalIntraMsg<Devices>>
{
  fn has_interface(self, interface: Self) -> bool;
}

/// Signifies that a type belongs to a [`UnifiedType`]
pub trait Case<S> {
  const VARIANT: Self;
}

/// Denotes message types that [`Actor`](crate::core::Actor) receives.
pub trait RootMessage<U: UnifiedType + Case<Self>>
where
  Self: Sized + Send + 'static,
{
  fn deserialize_as(
    interface: U,
    intp: Interpretations,
    bytes: &[u8],
  ) -> Result<LocalActorMsg<Self>, DeserializeError<U>>;

  fn has_interface(interface: U) -> bool;
}