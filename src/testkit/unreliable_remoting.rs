use crate::core::{
  ActorSignal, Case, Destination, Interpretations, MessagePackets, Socket,
  UnifiedBounds,
};
use crate::testkit::FailureConfigMap;
use itertools::Itertools;
use serde::de::DeserializeOwned;
use serde::Serialize;

pub async fn udp_msg_unreliable_msg<U: UnifiedBounds + Case<I>, I>(
  socket: &Socket,
  dest: &Destination<U, I>,
  msg: &I,
  fail_cfg: &FailureConfigMap,
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
  fail_cfg: &FailureConfigMap,
) {
  udp_unreliable_msg(&socket, &dest, Interpretations::Signal, sig, fail_cfg)
    .await;
}

async fn udp_unreliable_msg<U: UnifiedBounds + Case<I>, I, T>(
  socket: &Socket,
  dest: &Destination<U, I>,
  intp: Interpretations,
  msg: &T,
  fail_cfg: &FailureConfigMap,
) where
  T: Serialize + DeserializeOwned,
{
  if rand::random::<f64>() >= fail_cfg.get(socket).drop_prob {
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
  fail_cfg: &FailureConfigMap,
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
  fail_cfg: &FailureConfigMap,
) {
  udp_unreliable_packet(&socket, &dest, Interpretations::Signal, sig, fail_cfg)
    .await;
}

async fn udp_unreliable_packet<U: UnifiedBounds + Case<I>, I, T>(
  socket: &Socket,
  dest: &Destination<U, I>,
  intp: Interpretations,
  msg: &T,
  fail_map: &FailureConfigMap,
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
    .send_to_unreliable(&udp, addr, fail_map.get(socket))
    .await;
}

#[macro_export]
macro_rules! udp_select {
  ($reliable:expr, $fail_map:expr, $socket:expr, $dest:expr, $msg:expr) => {
    match $reliable {
      crate::testkit::FailureMode::None => {
        aurum::core::udp_msg($socket, $dest, $msg).await
      }
      crate::testkit::FailureMode::Message => {
        aurum::testkit::udp_msg_unreliable_msg($socket, $dest, $msg, $fail_map)
          .await
      }
      crate::testkit::FailureMode::Packet => {
        aurum::testkit::udp_msg_unreliable_packet(
          $socket, $dest, $msg, $fail_map,
        )
        .await
      }
    }
  };
}
