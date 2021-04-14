use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use std::hash::Hash;

use crate::core::{
  ActorRef, DeserializeError, Destination, Interpretations, LocalActorMsg,
  RegistryMsg, Socket,
};
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
    dest: Destination::new::<S, I>(name),
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
