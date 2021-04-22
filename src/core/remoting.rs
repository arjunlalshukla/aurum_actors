use crate::core::{
  ActorName, ActorSignal, Case, Interpretations, MessagePackets,
  SpecificInterface, UnifiedBounds,
};
use itertools::Itertools;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::hash::Hash;
use std::net::{IpAddr, SocketAddr};
use std::{cmp::PartialEq, marker::PhantomData};
use tokio::net::lookup_host;

#[derive(
  Clone, Debug, Deserialize, Eq, Hash, PartialEq, Ord, PartialOrd, Serialize,
)]
pub enum Host {
  DNS(String),
  IP(IpAddr),
}

#[derive(
  Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, Ord, PartialOrd,
)]
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

#[derive(Eq, PartialEq, Clone, Hash, Debug, Deserialize, Serialize)]
#[serde(bound = "U: Serialize + DeserializeOwned")]
pub struct DestinationUntyped<U: UnifiedBounds> {
  pub name: ActorName<U>,
  pub interface: U,
}

#[derive(Eq, Deserialize, Serialize)]
#[serde(bound = "U: Serialize + DeserializeOwned")]
pub struct Destination<U: UnifiedBounds + Case<I>, I> {
  pub untyped: DestinationUntyped<U>,
  pub x: PhantomData<I>,
}
impl<U: UnifiedBounds + Case<I>, I> Destination<U, I> {
  pub fn new<S>(s: String) -> Destination<U, I>
  where
    U: Case<S>,
    S: From<I> + SpecificInterface<U>,
  {
    Destination {
      untyped: DestinationUntyped {
        name: ActorName::new::<S>(s),
        interface: <U as Case<I>>::VARIANT,
      },
      x: PhantomData,
    }
  }

  pub fn name(&self) -> &ActorName<U> {
    &self.untyped.name
  }

  pub fn untyped(&self) -> &DestinationUntyped<U> {
    &self.untyped
  }
}
impl<U: UnifiedBounds + Case<I>, I> Clone for Destination<U, I> {
  fn clone(&self) -> Self {
    Destination {
      untyped: self.untyped.clone(),
      x: PhantomData,
    }
  }
}
impl<U: UnifiedBounds + Case<I>, I> PartialEq for Destination<U, I> {
  fn eq(&self, other: &Self) -> bool {
    self.untyped == other.untyped
  }
}
impl<U: UnifiedBounds + Case<I>, I> Hash for Destination<U, I> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.untyped.hash(state);
  }
}
impl<U: UnifiedBounds + Case<I>, I> Debug for Destination<U, I> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("ActorRef")
      .field("Unified", &std::any::type_name::<U>())
      .field("Interface", &std::any::type_name::<I>())
      .field("name", &self.untyped)
      .finish()
  }
}

pub async fn udp_msg<U: UnifiedBounds + Case<I>, I>(
  socket: &Socket,
  dest: &Destination<U, I>,
  msg: &I,
) where
  I: Serialize + DeserializeOwned,
{
  udp_send(&socket, &dest, Interpretations::Message, msg).await;
}

pub async fn udp_signal<U: UnifiedBounds + Case<I>, I>(
  socket: &Socket,
  dest: &Destination<U, I>,
  sig: &ActorSignal,
) {
  udp_send(&socket, &dest, Interpretations::Signal, sig).await;
}

async fn udp_send<U: UnifiedBounds + Case<I>, I, T>(
  socket: &Socket,
  dest: &Destination<U, I>,
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
    .move_to(&udp, addr)
    .await;
}
