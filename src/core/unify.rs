use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fmt::Debug;

use crate::core::{ActorRef, Address, HasInterface, Node, SpecificInterface};

pub trait Case<Specific> where Self: Clone + Sized + Serialize + DeserializeOwned + Debug {
  const VARIANT: Self;
  fn forge<T>(s: String, n: Node) -> ActorRef<Self, T> where 
   Specific: Serialize + DeserializeOwned + HasInterface<T> + SpecificInterface<Self>, 
   Self: Case<T>, T: Serialize + DeserializeOwned {
    ActorRef::new(
      Address::new(n, <Self as Case<Specific>>::VARIANT, s),
      <Self as Case<T>>::VARIANT,
      None)
  }
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