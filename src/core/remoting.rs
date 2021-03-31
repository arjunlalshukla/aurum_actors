use crate::core::{ActorSignal, Case, Interpretations, SpecificInterface};
use itertools::Itertools;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::cmp::PartialEq;
use std::fmt::Debug;
use std::hash::Hash;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
use tokio::net::lookup_host;

use super::{ActorName, MessagePackets, UnifiedBounds};

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
    .move_to(&udp, addr)
    .await;
}

pub async fn udp_msg_unreliable<U: UnifiedBounds, T>(
  socket: &Socket,
  dest: &Destination<U>,
  msg: &T,
  dur: &Option<(Duration, Duration)>,
  fail_prob: f64,
) where
  T: Serialize + DeserializeOwned,
{
  udp_unreliable(
    &socket,
    &dest,
    Interpretations::Message,
    msg,
    dur,
    fail_prob,
  )
  .await;
}

pub async fn udp_signal_unreliable<U: UnifiedBounds>(
  socket: &Socket,
  dest: &Destination<U>,
  sig: &ActorSignal,
  dur: &Option<(Duration, Duration)>,
  fail_prob: f64,
) {
  udp_unreliable(&socket, &dest, Interpretations::Signal, sig, dur, fail_prob)
    .await;
}

async fn udp_unreliable<U: UnifiedBounds, T>(
  socket: &Socket,
  dest: &Destination<U>,
  intp: Interpretations,
  msg: &T,
  dur: &Option<(Duration, Duration)>,
  fail_prob: f64,
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
    .send_to_unreliable(&udp, addr, dur, fail_prob)
    .await;
}
