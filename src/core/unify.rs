use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fmt::Debug;

use crate::core::{ActorRef, Address, HasInterface, Socket, SpecificInterface};


pub fn forge<Unified, Specific, Interface>(s: String, n: Socket)
 -> ActorRef<Unified, Interface> where 
 Specific: HasInterface<Interface> + SpecificInterface<Unified>, 
 Unified: Case<Specific> + Case<Interface> + Serialize + DeserializeOwned, 
 Interface: Serialize + DeserializeOwned {
  ActorRef::new(
    Address::new(n, <Unified as Case<Specific>>::VARIANT, s),
    <Unified as Case<Interface>>::VARIANT,
    None)
}

pub trait Case<Specific> where Self: Sized + Debug + Clone + Serialize + DeserializeOwned {
  const VARIANT: Self;
}  

// Haskell-style algebraic data types
#[macro_export]
macro_rules! unified {
  ($name:ident = $($part:ident)|*) => {
    #[derive(serde::Serialize, serde::Deserialize, std::cmp::Eq, 
      std::cmp::PartialEq, std::fmt::Debug, std::hash::Hash, std::clone::Clone
    )]
    enum $name {
      $($part,)*
    }
    $(
      impl aurum::core::Case<$part> for $name {
        const VARIANT: $name = $name::$part;
      }
    )*
  };
}