use crate as aurum_actors;
use crate::cluster::NodeRing;
use crate::core::{Host, Socket};
use crate::AurumInterface;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::Duration;
use ClusterEvent::*;

/// Configures a [`Cluster`](crate::cluster::Cluster).
#[derive(Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
  /// The length of time this [`Cluster`](crate::cluster::Cluster) is allowed to have without
  /// receiving a gossip instance before it must request one.
  /// 
  /// default: `1 second`
  pub gossip_timeout: Duration,
  /// The number of nodes this [`Cluster`](crate::cluster::Cluster) should send gossips to and
  /// request gossip from in addition to its neighbors in the [`NodeRing`].
  /// 
  /// default: `1`
  pub gossip_disperse: usize,
  /// The amount of time this [`Cluster`](crate::cluster::Cluster) waits after sending a ping to
  /// seed nodes before it either resends the ping or gives up and starts its own cluster.
  /// 
  /// default: `300 milliseconds`
  pub ping_timeout: Duration,
  /// The maxmimum number of pings this [`Cluster`](crate::cluster::Cluster) will send before
  /// giving up.
  /// 
  /// default: `5`
  pub num_pings: usize,
  /// The interval this [`Cluster`](crate::cluster::Cluster) will send heartbeats to its managers
  /// in the [`NodeRing`].
  /// 
  /// default: `50 milliseconds`
  pub hb_interval: Duration,
  /// The list of seed nodes this [`Cluster`](crate::cluster::Cluster) will contact to join a
  /// cluster.
  /// 
  /// default: `im::hashset[]`
  pub seed_nodes: im::HashSet<Socket>,
  /// The number of nodes each node in the cluster will be managed by. THIS VALUE MUST BE THE
  /// SAME FOR EVERY NODE IN THE CLUSTER.
  /// 
  /// default: `2`
  pub replication_factor: usize,
  /// The number of `vnodes` this [`Cluster`](crate::cluster::Cluster) will have in the
  /// cluster-wide [`NodeRing`].
  /// 
  /// default: `1`
  pub vnodes: u32,
  /// Determines whether the actor running this [`Cluster`](crate::cluster::Cluster) should be
  /// double or single threaded.
  /// 
  /// default: `false`
  pub double: bool,
  x: PhantomData<()>
}
impl Default for ClusterConfig {
  #[inline]
  fn default() -> Self {
    ClusterConfig {
      gossip_timeout: Duration::from_millis(1000),
      gossip_disperse: 1,
      ping_timeout: Duration::from_millis(300),
      num_pings: 5,
      hb_interval: Duration::from_millis(50),
      seed_nodes: im::hashset![],
      replication_factor: 2,
      vnodes: 1,
      double: false,
      x: PhantomData
    }
  }
}

/// Configures heartbeat receivers (HBRs) working for a [`Cluster`](crate::cluster::Cluster).
#[derive(Clone, Serialize, Deserialize)]
pub struct HBRConfig {
  /// The probability of a failure given the Gaussian distribution of heartbeats at which we start
  /// requesting heartbeats.
  /// 
  /// default: `0.995`
  pub phi: f64,
  /// The number of heartbeat receipts each HBR will store in its buffer to build its Gaussian 
  /// distribution.
  /// 
  /// default: `10`
  pub capacity: usize,
  /// Indicates how full the buffer should be when each HBR starts. For a full buffer, use
  /// [`HBRConfig.capacity / 2`](#field.capacity).
  /// 
  /// default: `5`
  pub times: usize,
  /// The maximum number of heartbeat requests each HBR sends before marking a node as down.
  /// 
  /// default: `3`
  pub req_tries: usize,
  /// The amount of time each HBR waits after sending a heartbeat request to the node it monitors.
  /// 
  /// default: `100 milliseconds`
  pub req_timeout: Duration,
  x: PhantomData<()>
}
impl Default for HBRConfig {
  #[inline]
  fn default() -> Self {
    HBRConfig {
      phi: 0.995,
      capacity: 10,
      times: 5,
      req_tries: 3,
      req_timeout: Duration::from_millis(100),
      x: PhantomData
    }
  }
}

#[derive(AurumInterface, Clone, Serialize, Deserialize, Hash, PartialEq, Eq, Debug)]
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

/// Updates received by subscribers to events sent by a [`Cluster`](crate::cluster::Cluster).
/// 
/// The first update each subscriber gets contains the identifier of the local node, no matter when
/// it subscribes.
#[derive(Clone)]
pub struct ClusterUpdate {
  /// A list of events that happened since the last update in chronological order: adds, removals,
  /// downings, and changes to the local node's identifier.
  pub events: Vec<ClusterEvent>,
  /// The set of active cluster member. This excludes members who have been downed.
  pub nodes: im::HashSet<Arc<Member>>,
  /// The current [`NodeRing`] used by the [`Cluster`](crate::cluster::Cluster) to delegate
  /// responsibility.
  pub ring: NodeRing,
}

/// Represents a member of a cluster, who may be monitored by [`Cluster`](crate::cluster::Cluster).
#[derive(Serialize, Deserialize, Hash, Eq, Clone, Ord, PartialOrd, Debug)]
pub struct Member {
  /// The [`Socket`](crate::core::Socket) this member receives remote messages on.
  pub socket: Socket,
  /// The unique identifier for this member. This is used to differentiate between different times
  /// the same node may have joined the cluster (for instance, in the event of an erroneous
  /// downing). 
  pub id: u64,
  /// The number of `vnodes` currently
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
      socket: Socket::new(Host::IP(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))), 0, 0),
      id: 0,
      vnodes: 0,
    }
  }
}
