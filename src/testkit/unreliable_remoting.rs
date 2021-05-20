use crate::core::{
  ActorSignal, Case, Destination, Interpretations, MessagePackets, Node,
  Socket, UnifiedType,
};
use crate::testkit::FailureConfigMap;
use itertools::Itertools;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::time::Duration;
use tokio::time::sleep;

pub async fn udp_msg_unreliable_msg<U: UnifiedType + Case<I>, I>(
  node: &Node<U>,
  socket: &Socket,
  dest: &Destination<U, I>,
  msg: &I,
  fail_cfg: &FailureConfigMap,
) where
  I: Serialize + DeserializeOwned,
{
  udp_unreliable_msg(
    node,
    socket,
    dest,
    Interpretations::Message,
    msg,
    fail_cfg,
  )
  .await;
}

pub async fn udp_signal_unreliable_msg<U: UnifiedType + Case<I>, I>(
  node: &Node<U>,
  socket: &Socket,
  dest: &Destination<U, I>,
  sig: &ActorSignal,
  fail_cfg: &FailureConfigMap,
) {
  udp_unreliable_msg(
    node,
    socket,
    dest,
    Interpretations::Signal,
    sig,
    fail_cfg,
  )
  .await;
}

async fn udp_unreliable_msg<U: UnifiedType + Case<I>, I, T>(
  node: &Node<U>,
  socket: &Socket,
  dest: &Destination<U, I>,
  intp: Interpretations,
  msg: &T,
  fail_map: &FailureConfigMap,
) where
  T: Serialize + DeserializeOwned,
{
  let fail_cfg = fail_map.get(socket);
  let addrs = socket.as_udp_addr().await.unwrap();
  let addr = addrs.into_iter()
    .next()
    .expect(format!("No resolution for {:?}", socket).as_str());
  let udp = tokio::net::UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, 0))
    .await
    .unwrap();
  let packets = MessagePackets::new(msg, intp, dest);
  let dur = fail_cfg.delay.map(|(min, max)| {
    let range = min.as_millis()..=max.as_millis();
    Duration::from_millis(SmallRng::from_entropy().gen_range(range) as u64)
  });
  // We need to to the serialization work, even if the send fails.
  if rand::random::<f64>() >= fail_cfg.drop_prob {
    if let Some(dur) = dur {
      node.rt().spawn(async move {
        sleep(dur).await;
        packets.move_to(&udp, &addr).await;
      });
    } else {
      packets.move_to(&udp, &addr).await;
    }
  }
}

pub async fn udp_msg_unreliable_packet<U: UnifiedType + Case<I>, I>(
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

pub async fn udp_signal_unreliable_packet<U: UnifiedType + Case<I>, I>(
  socket: &Socket,
  dest: &Destination<U, I>,
  sig: &ActorSignal,
  fail_cfg: &FailureConfigMap,
) {
  udp_unreliable_packet(&socket, &dest, Interpretations::Signal, sig, fail_cfg)
    .await;
}

async fn udp_unreliable_packet<U: UnifiedType + Case<I>, I, T>(
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
    .next()
    .expect(format!("No resolution for {:?}", socket).as_str());
  let udp = tokio::net::UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, 0))
    .await
    .unwrap();
  MessagePackets::new(msg, intp, dest)
    .send_to_unreliable(&udp, addr, fail_map.get(socket))
    .await;
}

#[macro_export]
macro_rules! udp_select {
  ($reliable:expr, $node:expr, $fail_map:expr, $socket:expr, $dest:expr, $msg:expr) => {
    match $reliable {
      $crate::testkit::FailureMode::None => {
        aurum::core::udp_msg($socket, $dest, $msg).await
      }
      $crate::testkit::FailureMode::Message => {
        aurum::testkit::udp_msg_unreliable_msg(
          $node, $socket, $dest, $msg, $fail_map,
        )
        .await
      }
      $crate::testkit::FailureMode::Packet => {
        aurum::testkit::udp_msg_unreliable_packet(
          $socket, $dest, $msg, $fail_map,
        )
        .await
      }
    }
  };
}
