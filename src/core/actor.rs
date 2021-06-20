use crate::core::{
  ActorRef, Case, Destination, LocalRef, MessageBuilder, Node, SerializedRecvr, RootMessage,
  UnifiedType,
};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

/// An identifier for actors unique in the registry of a [`Node`]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
#[serde(bound = "U: UnifiedType")]
pub struct ActorId<U> {
  recv_type: U,
  name: String,
}
impl<U: UnifiedType> ActorId<U> {
  pub fn new<T>(s: String) -> ActorId<U>
  where
    U: Case<T>,
  {
    ActorId {
      recv_type: <U as Case<T>>::VARIANT,
      name: s,
    }
  }

  pub fn recv_type(&self) -> U {
    self.recv_type
  }

  pub fn name(&self) -> &String {
    &self.name
  }
}

/// Defines how actors process messages.
/// 
/// Each actor runs on at least one asynchronous task. Messages are received and processed
/// atomically. Actors are lightweight and have good scalability locally.
/// [`Node`](crate::core::Node) uses the [`Tokio`] runtime to schedule its tasks.
/// Aurum operates under the same rules as Rust’s (and Tokio’s) asynchrony: tasks are IO-bound.
/// Code running in tasks is expected to `await` often, and starvation can occur if asynchronous
/// code performs heavy compute operations or synchronous IO.
/// 
/// It is recommended to implement [`Actor`] the [`async_trait`](async_trait::async_trait)
/// annotation. Rust cannot have async functions in traits yet, so this is a temporary work-around.
/// [`async_trait`](async_trait::async_trait) changes the type signatures of the member funtions
/// for [`Actor`]. Just pretend the trait looks like this instead of the monstrosity below:
/// 
/// ```ignore
/// #[async_trait]
/// pub trait Actor<U: Case<S> + UnifiedType, S: Send + RootMessage<U>>
/// where
/// Self: Send + 'static,
/// {
///   async fn pre_start(&mut self, _: &ActorContext<U, S>) {}
///   async fn recv(&mut self, ctx: &ActorContext<U, S>, msg: S);
///   async fn post_stop(&mut self, _: &ActorContext<U, S>) {}
/// }
/// ```
/// 
/// You'll need to use [`async_trait`](async_trait::async_trait) for every implementation of
/// [`Actor`].
/// 
/// ```
/// use async_trait::async_trait;
/// use aurum::AurumInterface;
/// use aurum::core::{Actor, ActorContext, ActorRef, Case, UnifiedType};
/// use serde::{Serialize, Deserialize};
/// 
/// #[derive(AurumInterface, Serialize, Deserialize)]
/// #[serde(bound = "U: UnifiedType")]
/// enum Ball<U: UnifiedType + Case<Ball<U>>> {
///   Ping(ActorRef<U, Self>),
///   Pong(ActorRef<U, Self>),
/// }
/// 
/// struct Player<U: UnifiedType + Case<Ball<U>>> {
///   initial_contact: Option<ActorRef<U, Ball<U>>>
/// }
/// #[async_trait]
/// impl<U: UnifiedType + Case<Ball<U>>> Actor<U, Ball<U>> for Player<U> {
///   async fn pre_start(&mut self, ctx: &ActorContext<U, Ball<U>>) {
///     if let Some(r) = &self.initial_contact {
///       r.remote_send(&ctx.node, &Ball::Ping(ctx.interface())).await;
///     }
///   }
///   async fn recv(&mut self, ctx: &ActorContext<U, Ball<U>>, msg: Ball<U>) {
///     match msg {
///       Ball::Ping(r) => {
///         r.remote_send(&ctx.node, &Ball::Pong(ctx.interface())).await;
///       }
///       Ball::Pong(r) => {
///         r.remote_send(&ctx.node, &Ball::Ping(ctx.interface())).await;
///       }
///     }
///   }
/// } 
/// ```
/// [`Tokio`]: https://docs.rs/tokio/
#[async_trait]
pub trait Actor<U: Case<S> + UnifiedType, S: Send + RootMessage<U>>
where
  Self: Send + 'static,
{
  async fn pre_start(&mut self, _: &ActorContext<U, S>) {}
  async fn recv(&mut self, ctx: &ActorContext<U, S>, msg: S);
  async fn post_stop(&mut self, _: &ActorContext<U, S>) {}
}

#[async_trait]
pub trait TimeoutActor<U: Case<S> + UnifiedType, S: Send + RootMessage<U>> {
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

pub fn local_actor_msg_convert<S: From<I>, I>(msg: LocalActorMsg<I>) -> LocalActorMsg<S> {
  match msg {
    LocalActorMsg::Msg(s) => LocalActorMsg::Msg(S::from(s)),
    LocalActorMsg::Signal(s) => LocalActorMsg::Signal(s),
  }
}

/// Contains contextual information needed by implementors of [`Actor`] to process messages.
pub struct ActorContext<U, S>
where
  U: Case<S> + UnifiedType,
  S: 'static + Send + RootMessage<U>,
{
  pub(in crate::core) tx: UnboundedSender<ActorMsg<U, S>>,
  pub name: ActorId<U>,
  pub node: Node<U>,
}
impl<U, S> ActorContext<U, S>
where
  U: Case<S> + UnifiedType,
  S: 'static + Send + RootMessage<U>,
{
  pub(in crate::core) fn create_local<T: Send>(
    sender: UnboundedSender<ActorMsg<U, S>>,
  ) -> LocalRef<T>
  where
    S: From<T>,
  {
    LocalRef {
      func: Arc::new(move |x: LocalActorMsg<T>| {
        sender.send(ActorMsg::Msg(local_actor_msg_convert(x))).is_ok()
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

  pub(in crate::core) fn ser_recvr(&self) -> SerializedRecvr<U> {
    let sender = self.tx.clone();
    Box::new(move |unified: U, mb: MessageBuilder| {
      sender.send(ActorMsg::Serial(unified, mb)).is_ok()
    })
  }
}
