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
use std::time::Duration;
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

#[derive(Deserialize, Serialize)]
#[serde(bound = "U: Serialize + DeserializeOwned")]
pub struct Destination<U: UnifiedBounds + Case<I>, I> {
  pub name: ActorName<U>,
  pub interface: U,
  pub x: PhantomData<I>,
}
impl<U: UnifiedBounds + Case<I>, I: Send> Destination<U, I> {
  pub fn new<S>(s: String) -> Destination<U, I>
  where
    U: Case<S>,
    S: From<I> + SpecificInterface<U>,
  {
    Destination {
      name: ActorName::new::<S>(s),
      interface: <U as Case<I>>::VARIANT,
      x: PhantomData,
    }
  }
}
impl<U: UnifiedBounds + Case<I>, I> Clone for Destination<U, I> {
  fn clone(&self) -> Self {
    Destination {
      name: self.name.clone(),
      interface: self.interface,
      x: PhantomData,
    }
  }
}
impl<U: UnifiedBounds + Case<I>, I> PartialEq for Destination<U, I> {
  fn eq(&self, other: &Self) -> bool {
    self.name == other.name && self.interface == other.interface
  }
}
impl<U: UnifiedBounds + Case<I>, I> Eq for Destination<U, I> {}
impl<U: UnifiedBounds + Case<I>, I> Hash for Destination<U, I> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.name.hash(state);
    self.interface.hash(state);
  }
}
impl<U: UnifiedBounds + Case<I>, I> Debug for Destination<U, I> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("ActorRef")
      .field("Unified", &std::any::type_name::<U>())
      .field("Interface", &std::any::type_name::<I>())
      .field("name", &self.name)
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

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct FailureConfig {
  pub drop_prob: f64,
  pub delay: Option<(Duration, Duration)>,
}
impl Default for FailureConfig {
  fn default() -> Self {
    FailureConfig {
      drop_prob: 0.0,
      delay: None,
    }
  }
}

pub enum FailureMode {
  Packet,
  Message,
  None,
}

pub async fn udp_msg_unreliable_msg<U: UnifiedBounds + Case<I>, I>(
  socket: &Socket,
  dest: &Destination<U, I>,
  msg: &I,
  fail_cfg: FailureConfig,
) where
  I: Serialize + DeserializeOwned,
{
  udp_unreliable_msg(&socket, &dest, Interpretations::Message, msg, fail_cfg)
    .await;
}

pub async fn udp_signal_unreliable_msg<U: UnifiedBounds + Case<I>, I>(
  socket: &Socket,
  dest: &Destination<U, I>,
  sig: &ActorSignal,
  fail_cfg: FailureConfig,
) {
  udp_unreliable_msg(&socket, &dest, Interpretations::Signal, sig, fail_cfg)
    .await;
}

async fn udp_unreliable_msg<U: UnifiedBounds + Case<I>, I, T>(
  socket: &Socket,
  dest: &Destination<U, I>,
  intp: Interpretations,
  msg: &T,
  fail_cfg: FailureConfig,
) where
  T: Serialize + DeserializeOwned,
{
  if rand::random::<f64>() >= fail_cfg.drop_prob {
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
}

pub async fn udp_msg_unreliable_packet<U: UnifiedBounds + Case<I>, I>(
  socket: &Socket,
  dest: &Destination<U, I>,
  msg: &I,
  fail_cfg: FailureConfig,
) where
  I: Serialize + DeserializeOwned,
{
  udp_unreliable_packet(
    &socket,
    &dest,
    Interpretations::Message,
    msg,
    fail_cfg,
  )
  .await;
}

pub async fn udp_signal_unreliable_packet<U: UnifiedBounds + Case<I>, I>(
  socket: &Socket,
  dest: &Destination<U, I>,
  sig: &ActorSignal,
  fail_cfg: FailureConfig,
) {
  udp_unreliable_packet(&socket, &dest, Interpretations::Signal, sig, fail_cfg)
    .await;
}

async fn udp_unreliable_packet<U: UnifiedBounds + Case<I>, I, T>(
  socket: &Socket,
  dest: &Destination<U, I>,
  intp: Interpretations,
  msg: &T,
  fail_cfg: FailureConfig,
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
    .send_to_unreliable(&udp, addr, fail_cfg)
    .await;
}

#[macro_export]
macro_rules! udp_select {
  ($reliable:expr, $fail_cfg:expr, $socket:expr, $dest:expr, $msg:expr) => {
    match $reliable {
      crate::core::FailureMode::None => {
        aurum::core::udp_msg($socket, $dest, $msg).await
      }
      crate::core::FailureMode::Message => {
        aurum::core::udp_msg_unreliable_msg($socket, $dest, $msg, $fail_cfg)
          .await
      }
      crate::core::FailureMode::Packet => {
        aurum::core::udp_msg_unreliable_packet($socket, $dest, $msg, $fail_cfg)
          .await
      }
    }
  };
}
