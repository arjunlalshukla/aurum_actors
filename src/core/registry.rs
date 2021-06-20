use crate as aurum;
use crate::core::{
  deserialize, Actor, ActorContext, ActorId, DestinationUntyped, MessageBuilder, UnifiedType,
  LOG_LEVEL,
};
use crate::{error, info, trace, warn, AurumInterface};
use async_trait::async_trait;
use hashbrown::{hash_map::Entry, HashMap};
use tokio::sync::oneshot::Sender;

pub type SerializedRecvr<U> = Box<dyn Fn(U, MessageBuilder) -> bool + Send>;

#[derive(AurumInterface)]
#[aurum(local)]
pub enum RegistryMsg<U: UnifiedType> {
  Forward(MessageBuilder),
  Register(ActorId<U>, SerializedRecvr<U>, Sender<()>),
  Deregister(ActorId<U>),
}

pub struct Registry<U: UnifiedType> {
  pub register: HashMap<ActorId<U>, SerializedRecvr<U>>,
}
impl<U: UnifiedType> Registry<U> {
  pub fn new() -> Registry<U> {
    Registry {
      register: HashMap::new(),
    }
  }
}
#[async_trait]
impl<U: UnifiedType> Actor<U, RegistryMsg<U>> for Registry<U> {
  async fn recv(&mut self, ctx: &ActorContext<U, RegistryMsg<U>>, msg: RegistryMsg<U>) {
    match msg {
      RegistryMsg::Forward(msg_builder) => {
        let packets = msg_builder.max_seq_num;
        let DestinationUntyped {
          name,
          interface,
        } = deserialize::<DestinationUntyped<U>>(msg_builder.dest()).unwrap();
        if let Some(recvr) = self.register.get(&name) {
          if !recvr(interface, msg_builder) {
            self.register.remove(&name);
            let log = format!("Forward failed, removing actor {:?}", name);
            warn!(LOG_LEVEL, ctx.node, log);
          } else {
            let log = format!("Forwarded {} packets to {:?}", packets, name);
            trace!(LOG_LEVEL, ctx.node, log);
          }
        } else {
          warn!(LOG_LEVEL, ctx.node, format!("Not in register: {:?}", name));
        }
      }
      RegistryMsg::Register(name, channel, confirmation) => match self.register.entry(name) {
        Entry::Occupied(o) => {
          let log = format!("Already registered: {:?}", o.key());
          error!(LOG_LEVEL, ctx.node, log);
        }
        Entry::Vacant(v) => {
          if let Err(_) = confirmation.send(()) {
            let log = format!("Register confirmation failed: {:?}", v.key());
            error!(LOG_LEVEL, ctx.node, log);
          } else {
            let log = format!("Adding actor to registry: {:?}", v.key());
            info!(LOG_LEVEL, ctx.node, log);
            v.insert(channel);
          }
        }
      },
      RegistryMsg::Deregister(name) => {
        let log = format!("Removing actor from registry: {:?}", name);
        info!(LOG_LEVEL, ctx.node, log);
        self.register.remove(&name);
      }
    }
  }
}
