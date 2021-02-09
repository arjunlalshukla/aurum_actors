use crate::actor_ref::{ActorRef, Node};
use serde::Serialize;
use serde::de::DeserializeOwned;


pub trait Case<Specific> where Self: Sized + Serialize + DeserializeOwned,
 Specific: Serialize + DeserializeOwned {
  fn forge(s: String, n: Node) -> ActorRef<Self, Specific>;
  fn name(s: String) -> Self;
  fn is_instance(&self) -> bool;
}  

// Haskell-style algebraic data types
#[macro_export]
macro_rules! unified {
  ($name:ident = $($part:ident)|*) => {
    #[derive(serde::Serialize, serde::Deserialize, std::cmp::Eq, 
      std::cmp::PartialEq, std::fmt::Debug, std::hash::Hash
    )]
    enum $name {
      $($part(std::string::String),)*
    }

    $(
      impl aurum::unify::Case<$part> for $name {
        fn forge(s: std::string::String, n: aurum::actor_ref::Node) -> 
          aurum::actor_ref::ActorRef<$name, $part> {
          aurum::actor_ref::ActorRef::new(
            aurum::actor_ref::RemoteRef::new(n, $name::$part(s)), 
            std::option::Option::None
          )
        }

        fn name(s: String) -> $name { $name::$part(s) }

        fn is_instance(&self) -> bool { matches!(self, $name::$part(_)) }
      }
    )*

  };
}