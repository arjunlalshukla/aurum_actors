use crate as aurum;
use crate::cluster::{ClusterMsg, HeartbeatReceiverMsg, IntraClusterMsg};
use crate::core::{Case, Socket};
use crate::AurumInterface;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

pub trait UnifiedBounds:
  crate::core::UnifiedBounds
  + Case<ClusterMsg<Self>>
  + Case<IntraClusterMsg<Self>>
  + Case<HeartbeatReceiverMsg>
{
}
impl<T> UnifiedBounds for T where
  T: crate::core::UnifiedBounds
    + Case<ClusterMsg<Self>>
    + Case<IntraClusterMsg<Self>>
    + Case<HeartbeatReceiverMsg>
{
}

pub struct ClusterConfig {
  pub gossip_timeout: Duration,
  pub gossip_disperse: usize,
  pub ping_timeout: Duration,
  pub num_pings: usize,
  pub hb_interval: Duration,
  pub seed_nodes: Vec<Socket>,
  pub replication_factor: usize,
}
impl Default for ClusterConfig {
  fn default() -> Self {
    ClusterConfig {
      gossip_timeout: Duration::from_millis(1000),
      gossip_disperse: 1,
      ping_timeout: Duration::from_millis(500),
      num_pings: 5,
      hb_interval: Duration::from_millis(50),
      seed_nodes: vec![],
      replication_factor: 3,
    }
  }
}

#[derive(
  AurumInterface, Clone, Serialize, Deserialize, Hash, PartialEq, Eq, Debug,
)]
pub enum ClusterEventSimple {
  Alone,
  Joined,
  Added(Socket),
  Removed(Socket),
  Left,
}
impl From<ClusterEvent> for ClusterEventSimple {
  fn from(e: ClusterEvent) -> Self {
    match e {
      ClusterEvent::Added(m) => Self::Added(m.socket.clone()),
      ClusterEvent::Removed(m) => Self::Removed(m.socket.clone()),
      ClusterEvent::Alone(_) => Self::Alone,
      ClusterEvent::Joined(_) => Self::Joined,
      ClusterEvent::Left => Self::Left,
    }
  }
}

#[derive(
  AurumInterface, Clone, Serialize, Deserialize, Hash, PartialEq, Eq, Debug,
)]
pub enum ClusterEvent {
  Alone(Arc<Member>),
  Joined(Arc<Member>),
  Added(Arc<Member>),
  Removed(Arc<Member>),
  Left,
}

#[derive(Serialize, Deserialize, Hash, Eq, Clone, Ord, PartialOrd, Debug)]
pub struct Member {
  pub socket: Socket,
  pub id: u64,
  pub vnodes: u32,
}
impl PartialEq for Member {
  fn eq(&self, other: &Self) -> bool {
    // Should pretty much always take this path. Branch prediction hints?
    if self.id != other.id {
      return false;
    }
    if self.socket != other.socket {
      return false;
    }
    self.vnodes == other.vnodes
  }
}
