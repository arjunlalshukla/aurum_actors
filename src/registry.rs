use std::collections::HashMap;
use std::hash::Hash;
use std::fmt::Debug;

use crate::actor::{Actor, ActorContext};
use crate::unify::Case;

pub type SerializedRecvr<Unified> = Box<dyn Fn(Unified, Vec<u8>) -> bool>;

pub enum RegistryMsg<Unified> {
  Forward(Unified, String, Unified, Vec<u8>),
  Register(Unified, String, SerializedRecvr<Unified>),
  Deregister(Unified, String)
}

pub struct Registry<Unified> {
  pub register: HashMap<(Unified, String), SerializedRecvr<Unified>>
}
impl<Unified> Actor<Unified, RegistryMsg<Unified>> for Registry<Unified> where
 Unified: Case<RegistryMsg<Unified>> + Eq + Hash + Debug {
  fn recv(&mut self, ctx: &ActorContext<Unified, RegistryMsg<Unified>>, msg: RegistryMsg<Unified>) {
    match msg {
      RegistryMsg::Forward(recv_type, name, interface, payload) => {
        let key = (recv_type, name);
        match self.register.get(&key) {
          Some(recvr) => {
            if !recvr(interface, payload) {
              self.register.remove(&key);
              println!("Forward message to {:?} failed, removing actor", key);
            } else {
              println!("Forwarded message to {:?}", key);
            }
          }
          None => { println!("Cannot send to {:?}, not in register", key); }
        }
      }
      RegistryMsg::Register(recv_type, name, channel) => {
        self.register.insert((recv_type, name), channel);
      }
      RegistryMsg::Deregister(recv_type, name) => { 
        self.register.remove(&(recv_type, name));
      }
    }
  }
}