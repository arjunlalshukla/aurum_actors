use crate::core::{Address, Case, DeserializeError};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::sync::Arc;

use super::UnifiedBounds;

pub type LocalRef<T> = Arc<dyn Fn(T) -> bool>;

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
pub struct ActorRef<Unified: UnifiedBounds, Specific> {
  addr: Address<Unified>,
  interface: Unified,
  #[serde(skip, default)]
  local: Option<LocalRef<Specific>>,
}
impl<Unified: UnifiedBounds + Case<Specific>, Specific>
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

impl<Unified: UnifiedBounds + Case<Specific>, Specific> Debug
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
