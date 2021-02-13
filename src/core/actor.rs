use std::sync::Arc;
use crate::core::{ActorRef, Address, Case, HasInterface, LocalRef,
  SerializedRecvr};
use crossbeam::channel::Sender;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub trait Actor<Unified: Clone + Case<Msg>, Msg> {
  fn pre_start(&mut self) {}
  fn recv(&mut self, ctx: &ActorContext<Unified, Msg>, msg: Msg);
  fn post_stop(&mut self) {}
}

pub enum HiddenInterface<Unified, Specific> {
  Msg(Specific),
  Serial(Unified, Vec<u8>)
}

pub struct ActorContext<Unified: Case<Specific> + Clone, Specific> {
  pub tx: Sender<HiddenInterface<Unified, Specific>>,
  pub address: Address<Unified>
}
impl<Unified, Specific> ActorContext<Unified, Specific>
 where Unified: Case<Specific> + Clone + 'static, Specific: 'static {
  pub fn local_ref<T>(&self) -> LocalRef<T> where Specific: From<T> + 'static {
    let sender = self.tx.clone();
    Arc::new(move |x: T| 
      sender.send(HiddenInterface::Msg(Specific::from(x))).is_ok())
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
      sender.send(HiddenInterface::Serial(unified, vec)).is_ok())
  }
}