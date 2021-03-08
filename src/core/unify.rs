use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use std::hash::Hash;

use crate::core::{
  ActorRef, Destination, HasInterface, Socket, SpecificInterface,
};

use super::{ActorName, RegistryMsg};
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

pub trait SerDe: Serialize + DeserializeOwned {}
impl<T: Serialize + DeserializeOwned> SerDe for T {}

pub fn forge<U, S, I>(s: String, socket: Socket) -> ActorRef<U, I>
where
  U: Case<S> + Case<I> + UnifiedBounds,
  S: HasInterface<I> + SpecificInterface<U>,
  I: Send + Serialize + DeserializeOwned,
{
  ActorRef {
    socket: socket,
    dest: Destination {
      name: ActorName::new::<S>(s),
      interface: <U as Case<I>>::VARIANT,
    },
    local: None,
  }
}

pub trait Case<S> {
  const VARIANT: Self;
}
