use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use std::hash::Hash;

use crate::core::{
  ActorRef, Destination, HasInterface, Socket, SpecificInterface,
};

use super::ActorName;
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
{
}
impl<
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
      + DeserializeOwned,
  > UnifiedBounds for T
{
}

pub trait SerDe: Serialize + DeserializeOwned {}
impl<T: Serialize + DeserializeOwned> SerDe for T {}

pub fn forge<U, S, Interface: Send>(
  s: String,
  socket: Socket,
) -> ActorRef<U, Interface>
where
  S: HasInterface<Interface> + SpecificInterface<U>,
  U: Case<S> + Case<Interface> + UnifiedBounds,
  Interface: Serialize + DeserializeOwned,
{
  ActorRef {
    socket: socket,
    dest: Destination {
      name: ActorName::new::<S>(s),
      interface: <U as Case<Interface>>::VARIANT,
    },
    local: None,
  }
}

pub trait Case<S>
where
  Self: UnifiedBounds,
{
  const VARIANT: Self;
}
