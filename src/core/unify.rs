use crate::core::{
  ActorRef, DeserializeError, Destination, Interpretations, LocalActorMsg,
  RegistryMsg, Socket,
};
use crate::testkit::LoggerMsg;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use std::hash::Hash;

pub trait UnifiedBounds:
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
  + Serialize
  + DeserializeOwned
  + PartialOrd
  + Ord
  + Case<RegistryMsg<Self>>
  + Case<LoggerMsg>
{
}
impl<T> UnifiedBounds for T where
  T: 'static
    + Send
    + Sync
    + Debug
    + Copy
    + Clone
    + PartialEq
    + Eq
    + Hash
    + Debug
    + Serialize
    + DeserializeOwned
    + PartialOrd
    + Ord
    + Case<RegistryMsg<Self>>
    + Case<LoggerMsg>
{
}

pub fn forge<U, S, I>(name: String, socket: Socket) -> ActorRef<U, I>
where
  U: Case<S> + Case<I> + UnifiedBounds,
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

pub trait SpecificInterface<U: Debug>
where
  Self: Sized,
{
  fn deserialize_as(
    interface: U,
    intp: Interpretations,
    bytes: &[u8],
  ) -> Result<LocalActorMsg<Self>, DeserializeError<U>>;
}
