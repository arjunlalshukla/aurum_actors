use crate::core::{Address, Case, DeserializeError};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::sync::Arc;

use super::UnifiedBounds;

//pub type LocalRef<T> = Arc<dyn Fn(T) -> bool + Send + Sync>;

#[derive(Clone)]
pub struct LocalRef<T: Send> {
  pub(crate) func: Arc<dyn Fn(T) -> bool + Send + Sync>,
}
impl<T: Send> LocalRef<T> {
  pub fn send(&self, item: T) -> bool {
    (&self.func)(item)
  }

  pub fn void() -> LocalRef<T> {
    LocalRef {
      func: Arc::new(|_| false),
    }
  }

  pub fn panic() -> LocalRef<T> {
    LocalRef {
      func: Arc::new(|_| {
        panic!("LocalRef<{}> is a panic", std::any::type_name::<T>())
      }),
    }
  }
}

pub trait HasInterface<T: Serialize + DeserializeOwned> {}

pub trait SpecificInterface<Unified: Debug>
where
  Self: Sized,
{
  fn deserialize_as(
    interface: Unified,
    bytes: Vec<u8>,
  ) -> Result<Self, DeserializeError<Unified>>;
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(bound = "Unified: Case<Specific> + Serialize + DeserializeOwned")]
pub struct ActorRef<Unified: UnifiedBounds, Specific: Send> {
  addr: Address<Unified>,
  interface: Unified,
  #[serde(skip, default)]
  local: Option<LocalRef<Specific>>,
}
impl<Unified: UnifiedBounds + Case<Specific>, Specific: Send>
  ActorRef<Unified, Specific>
{
  pub fn new(
    addr: Address<Unified>,
    interface: Unified,
    local: Option<LocalRef<Specific>>,
  ) -> ActorRef<Unified, Specific> {
    ActorRef {
      addr: addr,
      interface: interface,
      local: local,
    }
  }
}

impl<Unified: UnifiedBounds + Case<Specific>, Specific: Send> Debug
  for ActorRef<Unified, Specific>
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("ActorRef")
      .field("Unified", &std::any::type_name::<Unified>())
      .field("Specific", &std::any::type_name::<Specific>())
      .field("addr", &self.addr)
      .field("interface", &self.interface)
      .field("has_local", &self.local.is_some())
      .finish()
  }
}
