use crate::core::{
  ActorRef, Case, Destination, LocalRef, MessageBuilder, Node, SerializedRecvr,
  SpecificInterface, UnifiedType,
};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

#[derive(
  Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize,
)]
#[serde(bound = "U: UnifiedType")]
pub struct ActorName<U> {
  pub recv_type: U,
  pub name: String,
}
impl<U: UnifiedType> ActorName<U> {
  pub fn new<T>(s: String) -> ActorName<U>
  where
    U: Case<T>,
  {
    ActorName {
      recv_type: <U as Case<T>>::VARIANT,
      name: s,
    }
  }
}

#[async_trait]
pub trait Actor<U: Case<S> + UnifiedType, S: Send + SpecificInterface<U>> 
where
  Self: Send + 'static
{
  async fn pre_start(&mut self, _: &ActorContext<U, S>) {}
  async fn recv(&mut self, ctx: &ActorContext<U, S>, msg: S);
  async fn post_stop(&mut self, _: &ActorContext<U, S>) {}
}

#[async_trait]
pub trait TimeoutActor<U: Case<S> + UnifiedType, S: Send + SpecificInterface<U>>
{
  async fn pre_start(&mut self, _: &ActorContext<U, S>) -> Option<Duration> {
    None
  }
  async fn recv(&mut self, _: &ActorContext<U, S>, _: S) -> Option<Duration>;
  async fn post_stop(&mut self, _: &ActorContext<U, S>) -> Option<Duration> {
    None
  }
  async fn timeout(&mut self, _: &ActorContext<U, S>) -> Option<Duration>;
}

pub(crate) enum ActorMsg<U, S> {
  Msg(LocalActorMsg<S>),
  Serial(U, MessageBuilder),
  PrimaryRequest,
  Die,
}

#[derive(Copy, Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum ActorSignal {
  Term,
}

#[derive(Eq, Serialize, Deserialize)]
#[serde(bound = "S: Serialize + DeserializeOwned")]
pub enum LocalActorMsg<S> {
  Msg(S),
  Signal(ActorSignal),
}
impl<S: PartialEq> PartialEq for LocalActorMsg<S> {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (LocalActorMsg::Msg(a), LocalActorMsg::Msg(b)) => a == b,
      (LocalActorMsg::Signal(a), LocalActorMsg::Signal(b)) => a == b,
      _ => false,
    }
  }
}
impl<S: Debug> Debug for LocalActorMsg<S> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let s = match self {
      LocalActorMsg::Msg(s) => format!("Msg({:?})", s),
      LocalActorMsg::Signal(s) => format!("Signal({:?})", s),
    };
    f.debug_struct("ActorRef")
      .field("Specific", &std::any::type_name::<S>())
      .field("variant", &s)
      .finish()
  }
}

pub fn local_actor_msg_convert<S: From<I>, I>(
  msg: LocalActorMsg<I>,
) -> LocalActorMsg<S> {
  match msg {
    LocalActorMsg::Msg(s) => LocalActorMsg::Msg(S::from(s)),
    LocalActorMsg::Signal(s) => LocalActorMsg::Signal(s),
  }
}

pub struct ActorContext<U, S>
where
  U: Case<S> + UnifiedType,
  S: 'static + Send + SpecificInterface<U>,
{
  pub(crate) tx: UnboundedSender<ActorMsg<U, S>>,
  pub name: ActorName<U>,
  pub node: Node<U>,
}
impl<U, S> ActorContext<U, S>
where
  U: Case<S> + UnifiedType,
  S: 'static + Send + SpecificInterface<U>,
{
  pub(in crate::core) fn create_local<T: Send>(
    sender: UnboundedSender<ActorMsg<U, S>>,
  ) -> LocalRef<T>
  where
    S: From<T>,
  {
    LocalRef {
      func: Arc::new(move |x: LocalActorMsg<T>| {
        sender
          .send(ActorMsg::Msg(local_actor_msg_convert(x)))
          .is_ok()
      }),
    }
  }

  pub fn local_interface<T: Send>(&self) -> LocalRef<T>
  where
    S: From<T>,
  {
    Self::create_local::<T>(self.tx.clone())
  }

  pub fn interface<T: Send>(&self) -> ActorRef<U, T>
  where
    U: Case<T>,
    S: From<T>,
  {
    ActorRef {
      socket: self.node.socket().clone(),
      dest: Destination::new::<S>(self.name.name.clone()),
      local: Some(self.local_interface::<T>()),
    }
  }

  pub fn ser_recvr(&self) -> SerializedRecvr<U> {
    let sender = self.tx.clone();
    Box::new(move |unified: U, mb: MessageBuilder| {
      sender.send(ActorMsg::Serial(unified, mb)).is_ok()
    })
  }
}
