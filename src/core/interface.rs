use crate::core::{Case, DeserializeError, LocalActorMsg};
use itertools::Itertools;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{fmt::Debug, net::SocketAddr};

use super::{ActorName, MessagePackets, Socket, UnifiedBounds};

#[derive(Clone)]
pub struct LocalRef<T: Send> {
  pub(crate) func: Arc<dyn Fn(LocalActorMsg<T>) -> bool + Send + Sync>,
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

pub trait SpecificInterface<Unified: Debug>
where
  Self: Sized,
{
  fn deserialize_as(
    interface: Unified,
    bytes: &[u8],
  ) -> Result<LocalActorMsg<Self>, DeserializeError<Unified>>;
}

#[derive(Clone, Deserialize, Serialize, Debug)]
#[serde(bound = "Unified: Serialize + DeserializeOwned")]
pub struct Destination<Unified: UnifiedBounds> {
  pub name: ActorName<Unified>,
  pub interface: Unified,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(bound = "Unified: Case<Specific> + Serialize + DeserializeOwned")]
pub struct ActorRef<Unified, Specific>
where
  Unified: UnifiedBounds,
  Specific: Send + Serialize + DeserializeOwned,
{
  pub(in crate::core) socket: Socket,
  pub(in crate::core) dest: Destination<Unified>,
  #[serde(skip, default)]
  pub(in crate::core) local: Option<LocalRef<Specific>>,
}
impl<Unified, Specific> ActorRef<Unified, Specific>
where
  Unified: UnifiedBounds + Case<Specific>,
  Specific: Send + Serialize + DeserializeOwned,
{
  pub async fn send(&self, item: Specific) -> Option<bool> {
    if let Some(r) = &self.local {
      Some(r.send(item))
    } else {
      self.remote_send(item).await;
      None
    }
  }

  async fn remote_send(&self, item: Specific) {
    let socks = self.socket.as_udp_addr().await.unwrap();
    let sock: SocketAddr = match socks.iter().exactly_one() {
      Ok(x) => x.clone(),
      Err(_) => panic!("multiple addrs: {:?}", socks),
    };
    let udp = tokio::net::UdpSocket::bind("0.0.0.0:0").await.unwrap();
    MessagePackets::new(&LocalActorMsg::Msg(item), &self.dest)
      .send_to(&udp, &sock)
      .await;
  }
}
impl<Unified, Specific> Debug for ActorRef<Unified, Specific>
where
  Unified: UnifiedBounds + Case<Specific>,
  Specific: Send + Serialize + DeserializeOwned,
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("ActorRef")
      .field("Unified", &std::any::type_name::<Unified>())
      .field("Specific", &std::any::type_name::<Specific>())
      .field("socket", &self.socket)
      .field("dest", &self.dest)
      .field("has_local", &self.local.is_some())
      .finish()
  }
}
