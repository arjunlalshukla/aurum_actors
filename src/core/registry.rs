use crate as aurum;
use crate::core::{Actor, ActorContext, ActorName, Case, UnifiedBounds};
use std::collections::HashMap;

pub type SerializedRecvr<Unified> =
  Box<dyn Fn(Unified, Vec<u8>) -> bool + Send>;

pub enum RegistryMsg<Unified: UnifiedBounds> {
  Forward(ActorName<Unified>, Unified, Vec<u8>),
  Register(ActorName<Unified>, SerializedRecvr<Unified>),
  Deregister(ActorName<Unified>),
}
impl<Unified: UnifiedBounds> aurum::core::SpecificInterface<Unified>
  for RegistryMsg<Unified>
where
  Unified: Case<RegistryMsg<Unified>>,
{
  fn deserialize_as(
    item: Unified,
    _bytes: Vec<u8>,
  ) -> std::result::Result<Self, aurum::core::DeserializeError<Unified>> {
    std::result::Result::Err(
      aurum::core::DeserializeError::IncompatibleInterface(item),
    )
  }
}

pub struct Registry<Unified: UnifiedBounds> {
  pub register: HashMap<ActorName<Unified>, SerializedRecvr<Unified>>,
}
impl<Unified: UnifiedBounds> Registry<Unified> {
  pub fn new() -> Registry<Unified> {
    Registry {
      register: HashMap::new(),
    }
  }
}
impl<Unified: UnifiedBounds> Actor<Unified, RegistryMsg<Unified>>
  for Registry<Unified>
where
  Unified: Case<RegistryMsg<Unified>> + UnifiedBounds,
{
  fn recv(
    &mut self,
    _ctx: &ActorContext<Unified, RegistryMsg<Unified>>,
    msg: RegistryMsg<Unified>,
  ) {
    match msg {
      RegistryMsg::Forward(name, interface, payload) => {
        match self.register.get(&name) {
          Some(recvr) => {
            if !recvr(interface, payload) {
              self.register.remove(&name);
              println!("Forward message to {:?} failed, removing actor", name);
            } else {
              println!("Forwarded message to {:?}", name);
            }
          }
          None => {
            println!("Cannot send to {:?}, not in register", name);
          }
        }
      }
      RegistryMsg::Register(name, channel) => {
        println!("Adding actor to registry: {:?}", name);
        self.register.insert(name, channel);
      }
      RegistryMsg::Deregister(name) => {
        println!("Removing actor from registry: {:?}", name);
        self.register.remove(&name);
      }
    }
  }
}
