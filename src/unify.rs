use crate::actor_ref::{ActorRef, Node};

pub trait ForgeRef<Specific> where Self: Sized {
  fn forge(s: String, n: Node) -> ActorRef<Self, Specific>;
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
      impl aurum::unify::ForgeRef<$part> for $name {
        fn forge(s: std::string::String, n: aurum::actor_ref::Node) -> 
          aurum::actor_ref::ActorRef<$name, $part> {
          aurum::actor_ref::ActorRef::new(
            aurum::actor_ref::RemoteRef::new(n, $name::$part(s)), 
            std::option::Option::None
          )
        }
      }
    )*

  };
}