use crate::cluster::{ClusterMsg, HeartbeatReceiverMsg, IntraClusterMsg};
use crate::core::{
  ActorRef, DeserializeError, Destination, Interpretations, LocalActorMsg,
  RegistryMsg, Socket,
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
  + Debug
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
{
  fn has_interface(self, interface: Self) -> bool;
}

pub fn forge<U, S, I>(name: String, socket: Socket) -> ActorRef<U, I>
where
  U: Case<S> + Case<I> + UnifiedType,
  S: From<I> + SpecificInterface<U>,
  I: Send,
{
  ActorRef {
    socket: socket,
    dest: Destination::new::<S>(name),
    local: None,
  }
}

pub trait Case<S> {
  const VARIANT: Self;
}

pub trait SpecificInterface<U: UnifiedType>
where
  Self: Sized,
{
  fn deserialize_as(
    interface: U,
    intp: Interpretations,
    bytes: &[u8],
  ) -> Result<LocalActorMsg<Self>, DeserializeError<U>>;

  fn has_interface(interface: U) -> bool;
}
