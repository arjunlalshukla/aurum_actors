use crate::core::{Case, DeserializeError, LocalActorMsg};
use itertools::Itertools;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
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

  pub fn eager_kill(&self) -> bool {
    (&self.func)(LocalActorMsg::EagerKill)
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

#[derive(Clone, Deserialize, Serialize, Debug)]
#[serde(bound = "U: Serialize + DeserializeOwned")]
pub struct Destination<U: UnifiedBounds> {
  pub name: ActorName<U>,
  pub interface: U,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(bound = "U: Case<S> + Serialize + DeserializeOwned")]
pub struct ActorRef<U, S>
where
  U: UnifiedBounds,
  S: Send + Serialize + DeserializeOwned,
{
  pub(in crate::core) socket: Socket,
  pub(in crate::core) dest: Destination<U>,
  #[serde(skip, default)]
  pub(in crate::core) local: Option<LocalRef<S>>,
}
impl<U, S> ActorRef<U, S>
where
  U: UnifiedBounds + Case<S>,
  S: Send + Serialize + DeserializeOwned,
{
  pub fn local(&self) -> Option<LocalRef<S>> {
    self.local.clone()
  }

  pub async fn send(&self, item: S) -> Option<bool> {
    if let Some(r) = &self.local {
      Some(r.send(item))
    } else {
      self.remote_send(item).await;
      None
    }
  }

  async fn remote_send(&self, item: S) {
    let addrs = self.socket.as_udp_addr().await.unwrap();
    let addr = addrs
      .iter()
      .exactly_one()
      .expect(format!("multiple addrs: {:?}", addrs).as_str());
    let udp = tokio::net::UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, 0))
      .await
      .unwrap();
    MessagePackets::new(&LocalActorMsg::Msg(item), &self.dest)
      .send_to(&udp, addr)
      .await;
  }
}
impl<U, S> Debug for ActorRef<U, S>
where
  U: UnifiedBounds + Case<S>,
  S: Send + Serialize + DeserializeOwned,
{
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
