use crate as aurum;
use crate::core::{
  deserialize, Actor, ActorContext, ActorName, DestinationUntyped,
  MessageBuilder, UnifiedType, LOG_LEVEL,
};
use crate::{error, info, trace, warn, AurumInterface};
use async_trait::async_trait;
use std::collections::{hash_map::Entry, HashMap};
use tokio::sync::oneshot::Sender;

pub type SerializedRecvr<U> = Box<dyn Fn(U, MessageBuilder) -> bool + Send>;

#[derive(AurumInterface)]
#[aurum(local)]
pub enum RegistryMsg<U: UnifiedType> {
  Forward(MessageBuilder),
  Register(ActorName<U>, SerializedRecvr<U>, Sender<()>),
  Deregister(ActorName<U>),
}

pub struct Registry<U: UnifiedType> {
  pub register: HashMap<ActorName<U>, SerializedRecvr<U>>,
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
  async fn recv(
    &mut self,
    ctx: &ActorContext<U, RegistryMsg<U>>,
    msg: RegistryMsg<U>,
  ) {
    match msg {
      RegistryMsg::Forward(msg_builder) => {
        let packets = msg_builder.max_seq_num;
        let DestinationUntyped { name, interface } =
          deserialize::<DestinationUntyped<U>>(msg_builder.dest()).unwrap();
        if let Some(recvr) = self.register.get(&name) {
          if !recvr(interface, msg_builder) {
            self.register.remove(&name);
            warn!(
              LOG_LEVEL,
              ctx.node,
              format!("Message forward failed, removing actor {:?}", name)
            );
          } else {
            trace!(
              LOG_LEVEL,
              ctx.node,
              format!("Forwarded {} packets to {:?}", packets, name)
            );
          }
        } else {
          warn!(LOG_LEVEL, ctx.node, format!("Not in register: {:?}", name));
        }
      }
      RegistryMsg::Register(name, channel, confirmation) => {
        match self.register.entry(name) {
          Entry::Occupied(o) => {
            error!(
              LOG_LEVEL,
              ctx.node,
              format!("Already registered: {:?}", o.key())
            );
          }
          Entry::Vacant(v) => {
            if let Err(_) = confirmation.send(()) {
              error!(
                LOG_LEVEL,
                ctx.node,
                format!("Register confirmation failed: {:?}", v.key())
              );
            } else {
              info!(
                LOG_LEVEL,
                ctx.node,
                format!("Adding actor to registry: {:?}", v.key())
              );
              v.insert(channel);
            }
          }
        }
      }
      RegistryMsg::Deregister(name) => {
        info!(
          LOG_LEVEL,
          ctx.node,
          format!("Removing actor from registry: {:?}", name)
        );
        self.register.remove(&name);
      }
    }
  }
}
