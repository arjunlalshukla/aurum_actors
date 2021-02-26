use crate as aurum;
use crate::core::{
  Actor, ActorContext, ActorName, Case, Destination, MessageBuilder,
  UnifiedBounds,
};
use async_trait::async_trait;
use aurum_macros::AurumInterface;
use std::collections::HashMap;
use tokio::sync::oneshot::Sender;

pub type SerializedRecvr<Unified> =
  Box<dyn Fn(Unified, MessageBuilder) -> bool + Send>;

#[derive(AurumInterface)]
#[aurum(local)]
pub enum RegistryMsg<Unified: UnifiedBounds> {
  Forward(MessageBuilder),
  Register(ActorName<Unified>, SerializedRecvr<Unified>, Sender<()>),
  Deregister(ActorName<Unified>),
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
#[async_trait]
impl<Unified: UnifiedBounds> Actor<Unified, RegistryMsg<Unified>>
  for Registry<Unified>
where
  Unified: Case<RegistryMsg<Unified>> + UnifiedBounds,
{
  async fn recv(
    &mut self,
    _ctx: &ActorContext<Unified, RegistryMsg<Unified>>,
    msg: RegistryMsg<Unified>,
  ) {
    match msg {
      RegistryMsg::Forward(msg_builder) => {
        let Destination { name, interface } = match serde_json::from_slice::<
          Destination<Unified>,
        >(msg_builder.dest())
        {
          Ok(x) => x,
          Err(e) => panic!("Could not deserialize because: {:?}", e.classify()),
        };
        if let Some(recvr) = self.register.get(&name) {
          if !recvr(interface, msg_builder) {
            self.register.remove(&name);
            println!("Forward message to {:?} failed, removing actor", name);
          } else {
            //println!("Forwarded message to {:?}", name);
          }
        } else {
          println!("Cannot send to {:?}, not in register", name);
        }
      }
      RegistryMsg::Register(name, channel, confirmation) => {
        println!("Adding actor to registry: {:?}", name);
        self.register.insert(name.clone(), channel);
        if let Err(_) = confirmation.send(()) {
          println!("Could not send confirmation, removing from registry");
          self.register.remove(&name);
        }
      }
      RegistryMsg::Deregister(name) => {
        println!("Removing actor from registry: {:?}", name);
        self.register.remove(&name);
      }
    }
  }
}
