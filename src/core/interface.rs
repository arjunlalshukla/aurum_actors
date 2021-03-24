use crate::core::{
  local_actor_msg_convert, ActorSignal, Case, Interpretations, LocalActorMsg,
  SpecificInterface,
};
use itertools::Itertools;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::cmp::PartialEq;
use std::fmt::Debug;
use std::hash::Hash;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::net::lookup_host;

use super::{ActorName, MessagePackets, UnifiedBounds};

pub struct LocalRef<T: Send + 'static> {
  pub(crate) func: Arc<dyn Fn(LocalActorMsg<T>) -> bool + Send + Sync>,
}
impl<T: Send + 'static> Clone for LocalRef<T> {
  fn clone(&self) -> Self {
    LocalRef {
      func: self.func.clone(),
    }
  }
}
impl<T: Send + 'static> LocalRef<T> {
  pub fn send(&self, item: T) -> bool {
    (&self.func)(LocalActorMsg::Msg(item))
  }

  pub fn signal(&self, sig: ActorSignal) -> bool {
    (&self.func)(LocalActorMsg::Signal(sig))
  }

  pub fn transform<I: Send + 'static>(&self) -> LocalRef<I>
  where
    T: From<I>,
  {
    let func = self.func.clone();
    LocalRef {
      func: Arc::new(move |x: LocalActorMsg<I>| {
        func(local_actor_msg_convert(x))
      }),
    }
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

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum Host {
  DNS(String),
  IP(IpAddr),
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Socket {
  pub host: Host,
  pub udp: u16,
  pub tcp: u16,
}
impl Socket {
  pub fn new(host: Host, udp: u16, tcp: u16) -> Socket {
    Socket {
      host: host,
      udp: udp,
      tcp: tcp,
    }
  }

  pub async fn as_udp_addr(&self) -> std::io::Result<Vec<SocketAddr>> {
    match &self.host {
      Host::IP(ip) => Ok(vec![SocketAddr::new(*ip, self.udp)]),
      Host::DNS(s) => lookup_host((s.as_str(), self.udp))
        .await
        .map(|x| x.collect()),
    }
  }
}

#[derive(Clone, Eq, PartialEq, Deserialize, Hash, Serialize, Debug)]
#[serde(bound = "U: Serialize + DeserializeOwned")]
pub struct Destination<U: UnifiedBounds> {
  pub name: ActorName<U>,
  pub interface: U,
}
impl<U: UnifiedBounds> Destination<U> {
  pub fn new<S, I>(s: String) -> Destination<U>
  where
    U: Case<S> + Case<I> + UnifiedBounds,
    S: From<I> + SpecificInterface<U>,
    I: Send,
  {
    Destination {
      name: ActorName::new::<S>(s),
      interface: <U as Case<I>>::VARIANT,
    }
  }
}

pub async fn udp_msg<U: UnifiedBounds, T>(
  socket: &Socket,
  dest: &Destination<U>,
  msg: &T,
) where
  T: Serialize + DeserializeOwned,
{
  udp_send(&socket, &dest, Interpretations::Message, msg).await;
}

pub async fn udp_signal<U: UnifiedBounds>(
  socket: &Socket,
  dest: &Destination<U>,
  sig: &ActorSignal,
) {
  udp_send(&socket, &dest, Interpretations::Signal, sig).await;
}

async fn udp_send<U: UnifiedBounds, T>(
  socket: &Socket,
  dest: &Destination<U>,
  intp: Interpretations,
  msg: &T,
) where
  T: Serialize + DeserializeOwned,
{
  let addrs = socket.as_udp_addr().await.unwrap();
  let addr = addrs
    .iter()
    .exactly_one()
    .expect(format!("multiple addrs: {:?}", addrs).as_str());
  let udp = tokio::net::UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, 0))
    .await
    .unwrap();
  MessagePackets::new(msg, intp, dest)
    .send_to(&udp, addr)
    .await;
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(bound = "U: Serialize + DeserializeOwned")]
pub struct ActorRef<U: UnifiedBounds + Case<S>, S: Send + 'static> {
  pub(in crate::core) socket: Socket,
  pub(in crate::core) dest: Destination<U>,
  #[serde(skip, default)]
  pub(in crate::core) local: Option<LocalRef<S>>,
}
impl<U: UnifiedBounds + Case<S>, S: Send + 'static> ActorRef<U, S> {
  pub fn local(&self) -> &Option<LocalRef<S>> {
    &self.local
  }
}
impl<U: UnifiedBounds + Case<S>, S> ActorRef<U, S>
where
  S: Send + Serialize + DeserializeOwned + 'static,
{
  pub async fn send(&self, item: &S) -> Option<bool>
  where
    S: Clone,
  {
    if let Some(r) = &self.local {
      Some(r.send(item.clone()))
    } else {
      udp_msg(&self.socket, &self.dest, item).await;
      None
    }
  }

  pub async fn remote_send(&self, item: &S) {
    udp_msg(&self.socket, &self.dest, item).await;
  }

  pub async fn move_to(&self, item: S) -> Option<bool> {
    if let Some(r) = &self.local {
      Some(r.send(item))
    } else {
      udp_msg(&self.socket, &self.dest, &item).await;
      None
    }
  }

  pub async fn signal(&self, sig: ActorSignal) -> Option<bool> {
    if let Some(r) = &self.local {
      Some(r.signal(sig))
    } else {
      udp_signal(&self.socket, &self.dest, &sig).await;
      None
    }
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
