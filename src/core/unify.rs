use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use std::hash::Hash;

use crate::core::{ActorRef, Address, HasInterface, Socket, SpecificInterface};

use super::ActorName;
pub trait UnifiedBounds:
  'static
  + Send
  + Sync
  + Debug
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

pub fn forge<Unified, Specific, Interface: Send>(
  s: String,
  n: Socket,
) -> ActorRef<Unified, Interface>
where
  Specific: HasInterface<Interface> + SpecificInterface<Unified>,
  Unified: Case<Specific> + Case<Interface> + UnifiedBounds,
  Interface: Serialize + DeserializeOwned,
{
  ActorRef::new(
    Address::new::<Specific>(n, ActorName::new::<Specific>(s)),
    <Unified as Case<Interface>>::VARIANT,
    None,
  )
}

pub trait Case<Specific>
where
  Self: UnifiedBounds,
{
  const VARIANT: Self;
}
