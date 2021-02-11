use crate::actor::{ActorRef, RemoteRef, Node};
use serde::Serialize;
use serde::de::DeserializeOwned;


pub trait Case<Specific> where Self: Sized + Serialize + DeserializeOwned {
  const VARIANT: Self;
  fn forge(s: String, n: Node) -> ActorRef<Self, Specific> 
   where Specific: Serialize + DeserializeOwned {
    ActorRef::new(RemoteRef::new(n, Self::VARIANT, s), None)
  }
}  

// Haskell-style algebraic data types
#[macro_export]
macro_rules! unified {
  ($name:ident = $($part:ident)|*) => {
    #[derive(serde::Serialize, serde::Deserialize, std::cmp::Eq, 
      std::cmp::PartialEq, std::fmt::Debug, std::hash::Hash
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