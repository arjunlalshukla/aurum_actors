use std::sync::Arc;
use std::fmt::Debug;
use std::hash::Hash;
use crate::core::{ActorRef, Address, Case, HasInterface, LocalRef,
  Node, SerializedRecvr, UnifiedBounds};
use crossbeam::channel::Sender;
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(bound = "Unified: UnifiedBounds")]
pub struct ActorName<Unified>(Unified, String);
impl<Unified: UnifiedBounds> ActorName<Unified> {
  pub fn new<T>(s: String) -> ActorName<Unified> where Unified: Case<T> {
    ActorName(<Unified as Case<T>>::VARIANT, s)
  }
}

pub trait Actor<Unified: Case<Msg> + UnifiedBounds, Msg> {
  fn pre_start(&mut self) {}
  fn recv(&mut self, ctx: &ActorContext<Unified, Msg>, msg: Msg);
  fn post_stop(&mut self) {}
}

pub enum ActorMsg<Unified, Specific> {
  Msg(Specific),
  Serial(Unified, Vec<u8>)
}

pub struct ActorContext<Unified: Case<Specific> + UnifiedBounds, Specific> {
  pub tx: Sender<ActorMsg<Unified, Specific>>,
  pub address: Address<Unified>
}
impl<Unified: Case<Specific> + UnifiedBounds, Specific: 'static>
ActorContext<Unified, Specific> {
  pub fn local_ref<T>(&self) -> LocalRef<T> 
  where Specific: From<T> + 'static {
    let sender = self.tx.clone();
    Arc::new(move |x: T| 
      sender.send(ActorMsg::Msg(Specific::from(x))).is_ok())
  }

  pub fn interface<T>(&self) -> ActorRef<Unified, T> where
   Unified: Case<T>,
   T: Serialize + DeserializeOwned,
   Specific: HasInterface<T> + From<T> + 'static {
    ActorRef::new(
      self.address.clone(),
      <Unified as Case<T>>::VARIANT,
      Some(self.local_ref::<T>()))
  }

  pub fn ser_recvr(&self) -> SerializedRecvr<Unified> {
    let sender = self.tx.clone();
    Box::new(move |unified: Unified, vec: Vec<u8>| 
      sender.send(ActorMsg::Serial(unified, vec)).is_ok())
  }
}