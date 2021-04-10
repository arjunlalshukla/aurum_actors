use crate::core::{
  ActorRef, Case, Destination, LocalRef, MessageBuilder, Node, SerializedRecvr,
  UnifiedBounds,
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
#[serde(bound = "U: UnifiedBounds")]
pub struct ActorName<U>(U, String);
impl<U: UnifiedBounds> ActorName<U> {
  pub fn new<T>(s: String) -> ActorName<U>
  where
    U: Case<T>,
  {
    ActorName(<U as Case<T>>::VARIANT, s)
  }
}

#[async_trait]
pub trait Actor<U: Case<Msg> + UnifiedBounds, Msg: Send> {
  async fn pre_start(&mut self, _: &ActorContext<U, Msg>) {}
  async fn recv(&mut self, ctx: &ActorContext<U, Msg>, msg: Msg);
  async fn post_stop(&mut self, _: &ActorContext<U, Msg>) {}
}

#[async_trait]
pub trait TimeoutActor<U: Case<M> + UnifiedBounds, M: Send> {
  async fn pre_start(&mut self, _: &ActorContext<U, M>) -> Option<Duration> {
    None
  }
  async fn recv(&mut self, _: &ActorContext<U, M>, _: M) -> Option<Duration>;
  async fn post_stop(&mut self, _: &ActorContext<U, M>) -> Option<Duration> {
    None
  }
  async fn timeout(&mut self, _: &ActorContext<U, M>) -> Option<Duration>;
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

pub struct ActorContext<U: Case<S> + UnifiedBounds, S> {
  pub(crate) tx: UnboundedSender<ActorMsg<U, S>>,
  pub name: ActorName<U>,
  pub node: Node<U>,
}
impl<U: Case<S> + UnifiedBounds, S: 'static + Send> ActorContext<U, S> {
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
      dest: Destination {
        name: self.name.clone(),
        interface: <U as Case<T>>::VARIANT,
      },
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
