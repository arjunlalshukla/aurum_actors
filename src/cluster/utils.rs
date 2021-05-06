use crate as aurum;
use crate::cluster::NodeRing;
use crate::core::{Host, Socket};
use crate::AurumInterface;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::Duration;
use ClusterEvent::*;

#[derive(Clone, Serialize, Deserialize)]
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
      ping_timeout: Duration::from_millis(300),
      num_pings: 5,
      hb_interval: Duration::from_millis(50),
      seed_nodes: vec![],
      replication_factor: 2,
    }
  }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct HBRConfig {
  pub phi: f64,
  pub capacity: usize,
  pub times: usize,
  pub req_tries: usize,
  pub req_timeout: Duration,
}
impl Default for HBRConfig {
  fn default() -> Self {
    HBRConfig {
      phi: 0.995,
      capacity: 10,
      times: 5,
      req_tries: 3,
      req_timeout: Duration::from_millis(100),
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
impl ClusterEvent {
  pub fn end(&self) -> bool {
    matches!(self, Alone(_) | Joined(_) | Left)
  }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ClusterUpdate {
  pub events: Vec<ClusterEvent>,
  pub nodes: im::HashSet<Arc<Member>>,
  pub ring: NodeRing,
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
impl Default for Member {
  fn default() -> Self {
    Member {
      socket: Socket::new(
        Host::IP(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))),
        0,
        0,
      ),
      id: 0,
      vnodes: 0,
    }
  }
}
