use std::fmt::Debug;
use std::sync::Arc;
use crate::core::{Address, Case, DeserializeError};
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;

pub type LocalRef<T> = Arc<dyn Fn(T) -> bool>;

pub trait HasInterface<T: Serialize + DeserializeOwned> {}

pub trait SpecificInterface<Unified: Debug> where Self: Sized {
  fn deserialize_as(interface: Unified, bytes: Vec<u8>) -> 
    Result<Self, DeserializeError<Unified>>;
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(bound = "Unified: Case<Specific> + Serialize + DeserializeOwned")]
pub struct ActorRef<Unified, Specific> where Unified: Clone,
 Specific: Serialize + DeserializeOwned {
  addr: Address<Unified>,
  interface: Unified,
  #[serde(skip, default)]
  local: Option<LocalRef<Specific>>
}
impl<Unified, Specific> ActorRef<Unified, Specific> where 
 Unified: Clone + Case<Specific> + Serialize + DeserializeOwned,
 Specific: Serialize + DeserializeOwned {
  pub fn new(
    addr: Address<Unified>,
    interface: Unified,
    local: Option<LocalRef<Specific>>
  ) -> ActorRef<Unified, Specific> {
    ActorRef {addr: addr, interface: interface, local: local}
  }

  /* 
  fn new_interface<Interface>(&self) -> ActorRef<Unified, Interface> where 
   Unified: Clone + Case<Specific> + Case<Interface> + Serialize + DeserializeOwned + Debug,
   Specific: Serialize + DeserializeOwned + SpecificInterface<Unified> + HasInterface<Interface>, 
   Interface: Serialize + DeserializeOwned {
    ActorRef::new(self.addr.clone(), <Unified as Case<Interface>>::VARIANT, None)
  }
  */
}

impl<Unified, Specific> Debug for ActorRef<Unified, Specific> where 
 Unified: Clone + Case<Specific> + Serialize + DeserializeOwned + Debug,
 Specific: Serialize + DeserializeOwned{
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