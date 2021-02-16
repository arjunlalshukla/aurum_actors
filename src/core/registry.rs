use std::collections::HashMap;
use crate::core::{Actor, ActorContext, ActorName, Case, UnifiedBounds};

pub type SerializedRecvr<Unified> = Box<dyn Fn(Unified, Vec<u8>) -> bool>;

pub enum RegistryMsg<Unified: UnifiedBounds> {
  Forward(ActorName<Unified>, Unified, Vec<u8>),
  Register(ActorName<Unified>, SerializedRecvr<Unified>),
  Deregister(ActorName<Unified>)
}

pub struct Registry<Unified: UnifiedBounds> {
  pub register: HashMap<ActorName<Unified>, SerializedRecvr<Unified>>
}
impl<Unified: UnifiedBounds> Actor<Unified, RegistryMsg<Unified>>
 for Registry<Unified> where
 Unified: Case<RegistryMsg<Unified>> + UnifiedBounds {
  fn recv(&mut self, _ctx: &ActorContext<Unified, RegistryMsg<Unified>>, 
   msg: RegistryMsg<Unified>) {
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
        self.register.insert(name, channel);
      }
      RegistryMsg::Deregister(name) => { 
        self.register.remove(&name);
      }
    }
  }
}