use crate::core::{
  ActorRef, Address, Case, HasInterface, LocalRef, Node, SerializedRecvr,
  UnifiedBounds,
};
use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;

use super::RegistryMsg;

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(bound = "Unified: UnifiedBounds")]
pub struct ActorName<Unified>(Unified, String);
impl<Unified: UnifiedBounds> ActorName<Unified> {
  pub fn new<T>(s: String) -> ActorName<Unified>
  where
    Unified: Case<T>,
  {
    ActorName(<Unified as Case<T>>::VARIANT, s)
  }
}

#[async_trait]
pub trait Actor<Unified: Case<Msg> + UnifiedBounds, Msg: Send> {
  async fn pre_start(&mut self, _: &ActorContext<Unified, Msg>) {}
  async fn recv(&mut self, _: &ActorContext<Unified, Msg>, _: Msg);
  async fn post_stop(&mut self, _: &ActorContext<Unified, Msg>) {}
}

pub enum ActorMsg<Unified, Specific> {
  Msg(Specific),
  Serial(Unified, Vec<u8>),
}

pub struct ActorContext<Unified: Case<Specific> + UnifiedBounds, Specific> {
  pub tx: UnboundedSender<ActorMsg<Unified, Specific>>,
  pub name: ActorName<Unified>,
  pub node: Node<Unified>,
}
impl<Unified: Case<Specific> + UnifiedBounds, Specific: 'static + Send>
  ActorContext<Unified, Specific>
{
  pub(in crate::core) fn create_local<T: Send>(
    sender: UnboundedSender<ActorMsg<Unified, Specific>>,
  ) -> LocalRef<T>
  where
    Specific: From<T> + 'static,
  {
    LocalRef {
      func: Arc::new(move |x: T| {
        sender.send(ActorMsg::Msg(Specific::from(x))).is_ok()
      }),
    }
  }

  pub fn local_interface<T: Send>(&self) -> LocalRef<T>
  where
    Specific: From<T> + 'static,
  {
    Self::create_local::<T>(self.tx.clone())
  }

  pub fn interface<T: Send + Serialize + DeserializeOwned>(
    &self,
  ) -> ActorRef<Unified, T>
  where
    Unified: Case<T> + Case<RegistryMsg<Unified>>,
    Specific: HasInterface<T> + From<T> + 'static,
  {
    ActorRef::new(
      Address::new::<Specific>(self.node.socket().clone(), self.name.clone()),
      <Unified as Case<T>>::VARIANT,
      Some(self.local_interface::<T>()),
    )
  }

  pub fn ser_recvr(&self) -> SerializedRecvr<Unified> {
    let sender = self.tx.clone();
    Box::new(move |unified: Unified, vec: Vec<u8>| {
      sender.send(ActorMsg::Serial(unified, vec)).is_ok()
    })
  }
}
