use crate::core::{ActorSignal, Case, DeserializeError, LocalActorMsg};
use itertools::Itertools;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::cmp::PartialEq;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;

use super::{ActorName, MessagePackets, Socket, UnifiedBounds};

pub struct LocalRef<T: Send> {
  pub(crate) func: Arc<dyn Fn(LocalActorMsg<T>) -> bool + Send + Sync>,
}
impl<T: Send> Clone for LocalRef<T> {
  fn clone(&self) -> Self {
    LocalRef {
      func: self.func.clone(),
    }
  }
}
impl<T: Send> LocalRef<T> {
  pub fn send(&self, item: T) -> bool {
    (&self.func)(LocalActorMsg::Msg(item))
  }

  pub fn signal(&self, sig: ActorSignal) -> bool {
    (&self.func)(LocalActorMsg::Signal(sig))
  }

  pub fn void() -> LocalRef<T> {
    LocalRef {
      func: Arc::new(|_| false),
    }
  }

  pub fn panic() -> LocalRef<T> {
    LocalRef {
      func: Arc::new(|_| {
        panic!("LocalRef<{}> is a panic", std::any::type_name::<T>())
      }),
    }
  }
}

pub trait HasInterface<T: Serialize + DeserializeOwned> {}

pub trait SpecificInterface<U: Debug>
where
  Self: Sized,
{
  fn deserialize_as(
    interface: U,
    bytes: &[u8],
  ) -> Result<LocalActorMsg<Self>, DeserializeError<U>>;
}

#[derive(Clone, Eq, PartialEq, Deserialize, Hash, Serialize, Debug)]
#[serde(bound = "U: Serialize + DeserializeOwned")]
pub struct Destination<U: UnifiedBounds> {
  pub name: ActorName<U>,
  pub interface: U,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(bound = "U: Serialize + DeserializeOwned")]
pub struct ActorRef<U: UnifiedBounds + Case<S>, S: Send> {
  pub(in crate::core) socket: Socket,
  pub(in crate::core) dest: Destination<U>,
  #[serde(skip, default)]
  pub(in crate::core) local: Option<LocalRef<S>>,
}
impl<U: UnifiedBounds + Case<S>, S: Send> ActorRef<U, S> {
  pub fn local(&self) -> &Option<LocalRef<S>> {
    &self.local
  }
}
impl<U: UnifiedBounds + Case<S>, S> ActorRef<U, S>
where
  S: Send + Serialize + DeserializeOwned,
{
  pub async fn send(&self, item: S) -> Option<bool> {
    if let Some(r) = &self.local {
      Some(r.send(item))
    } else {
      self.remote_send(LocalActorMsg::Msg(item)).await;
      None
    }
  }

  pub async fn signal(&self, sig: ActorSignal) -> Option<bool> {
    if let Some(r) = &self.local {
      Some(r.signal(sig))
    } else {
      self.remote_send(LocalActorMsg::Signal(sig)).await;
      None
    }
  }

  async fn remote_send(&self, msg: LocalActorMsg<S>) {
    let addrs = self.socket.as_udp_addr().await.unwrap();
    let addr = addrs
      .iter()
      .exactly_one()
      .expect(format!("multiple addrs: {:?}", addrs).as_str());
    let udp = tokio::net::UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, 0))
      .await
      .unwrap();
    MessagePackets::new(&msg, &self.dest)
      .send_to(&udp, addr)
      .await;
  }
}
impl<U: UnifiedBounds + Case<S>, S: Send> PartialEq for ActorRef<U, S> {
  fn eq(&self, other: &Self) -> bool {
    self.socket == other.socket && self.dest == other.dest
  }
}
impl<U: UnifiedBounds + Case<S>, S: Send> Eq for ActorRef<U, S> {}
impl<U: UnifiedBounds + Case<S>, S: Send> Hash for ActorRef<U, S> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.socket.hash(state);
    self.dest.hash(state);
  }
}
impl<U: UnifiedBounds + Case<S>, S: Send> Debug for ActorRef<U, S> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("ActorRef")
      .field("Unified", &std::any::type_name::<U>())
      .field("Specific", &std::any::type_name::<S>())
      .field("socket", &self.socket)
      .field("dest", &self.dest)
      .field("has_local", &self.local.is_some())
      .finish()
  }
}
