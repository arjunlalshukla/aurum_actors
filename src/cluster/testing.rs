use crate::core::{ActorRef};
use lazy_static::lazy_static;
use std::env::var;
use std::time::Duration;

lazy_static! {
  pub(crate) static ref PACKET_DROP: f64 = var("AURUM_PACKET_DROP")
    .map(|x| x.parse().ok())
    .ok()
    .flatten()
    .unwrap_or(0.0);
  
  pub(crate) static ref DELAY: Option<(Duration, Duration)> = var("AURUM_MIN_DELAY")
    .map(|x| x.parse().ok().map(Duration::from_millis))
    .ok()
    .flatten()
    .zip(
      var("AURUM_MAX_DELAY")
        .map(|x| x.parse().ok().map(Duration::from_millis))
        .ok()
        .flatten()
    )
    .filter(|(x, y)| x <= y);
  }

#[macro_export]
macro_rules! actor_send {
  ($reliable:expr, $actor:expr, $msg:expr) => {
    if ($reliable) {
      $actor.remote_send($msg)
    } else {
      $actor.remote_unreliable($msg, DELAY, *PACKET_DROP)
    }
  };
}

#[macro_export]
macro_rules! udp_send {
  ($reliable:expr, $socket:expr, $dest:expr, $msg:expr) => {
    if ($reliable) {
      udp_msg($socket, $dest, $msg).await
    } else {
      aurum::core::udp_msg_unreliable(
        $socket, 
        $dest, 
        $msg, 
        &*aurum::cluster::DELAY, 
        *aurum::cluster::PACKET_DROP
      )
      .await
    }
  };
}