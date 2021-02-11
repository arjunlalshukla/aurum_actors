use crate::actor::{ActorRef, Address, HasInterface, Node};
use serde::Serialize;
use serde::de::DeserializeOwned;


pub trait Case<Specific> where Self: Clone + Sized + Serialize + DeserializeOwned {
  const VARIANT: Self;
  fn forge<T>(s: String, n: Node) -> ActorRef<Self, T> where 
   Specific: Serialize + DeserializeOwned + HasInterface<T>, 
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
      impl aurum::unify::Case<$part> for $name {
        const VARIANT: $name = $name::$part;
      }
    )*
  };
}