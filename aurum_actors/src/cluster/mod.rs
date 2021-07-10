//! Define a group of [`Node`], who all have knowledge of each other.
//! 
//! Knowledge of which [`Node`] are part of the cluster is propagated to other nodes via a gossip
//! protocol, where information on on [`Node`] and their state is randomly disseminated until every
//! node in the cluster has it. If a node has not received a piece of gossip for a while, it will
//! request one. The nodes all keep track of each other's state, detect failures of other nodes, and
//! propogate knowledge of those failures with gossip.
//! 
//! ### Failure Detection
//! An [`Aurum`] cluster is elastic: nodes can be added or removed from a cluster at any time, and
//! the system will adapt. Should a node fail, it will be noticed through a probabilistic failure
//! detection. Each node keeps a buffer of timestamps on heartbeats it has received from the nodes
//! it is in charge of monitoring. The gaps between these timestamps are arranged in a Gaussian 
//! distribution. A probability `phi` (given by the user as a configuration option) will be compared
//! against the
//! [cumulative distribution function](https://en.wikipedia.org/wiki/Cumulative_distribution_function)
//! (CDF) of the time since the last received heartbeat to determine if the node should be
//! considered down. When the time since the last heartbeat reaches a value when the CDF is at or
//! above phi, the node will be marked down. The state change will then be dispersed throughout the
//! cluster with gossip. In the event of an erroneous downing, the node being downed will eventually
//! receive word that is has been downed (through gossip requests). It will assign itself a new
//! identifier and rejoin the cluster.
//! 
//! ### Delegating Responsibility
//! Nodes do not track every other node, but a small subset of them. The alternative would lead to a
//! quadratic increase in network traffic as the cluster grew. Instead, nodes are hashed and placed
//! within a [consistent hashing](https://en.wikipedia.org/wiki/Consistent_hashing) ring 
//! (a [`NodeRing`] in code). The [`NodeRing`] has an inverse
//! function, so nodes can see which nodes are in charge of them, and which nodes they have to
//! monitor. The gossip protocol is heavily biased towards sending to a node's neighbors in the
//! ring, because ring states must agree with each other if heartbeats are to work properly. When a
//! node is added, removed or downed (therefore changing the ring), only that node's neighbors in
//! the ring are affected. Using the ring, the entire cluster need not change its monitoring
//! behavior. You will see references to `vnodes` pop up thoughout the rest of this documentation
//! for [`cluster`](crate::cluster). See [`NodeRing`] for an explaination.
//! 
//! ### Using [`cluster`](crate::cluster)
//! Joining a cluster requires creation of a local cluster instance with [`Cluster`]. Local actors
//! can subscribe to changes in the state of the local cluster instance (carried in a
//! [`ClusterUpdate`]). The first update each subscriber gets (no matter when it subscribes)
//! contains the identifier of the local node. Because subscribers have full access to the node
//! ring, they can use it to perform their own sharding. Any hashable value can ask the ring which
//! nodes should be responsible for it.
//! 
//! [`Node`]: crate::core::Node
//! [`Aurum`]: crate

use crate::testkit::{FailureMode, LogLevel};
mod cluster;
pub mod crdt;
pub mod devices;
mod gossip;
mod heartbeat_receiver;
mod interval_storage;
mod node_ring;
mod utils;

pub const FAILURE_MODE: FailureMode = FailureMode::Message;
pub const LOG_LEVEL: LogLevel = LogLevel::Warn;

#[rustfmt::skip]
pub(crate) use {
  cluster::NodeState,
  gossip::Gossip,
  gossip::MachineState,
  heartbeat_receiver::HeartbeatReceiver,
  interval_storage::IntervalStorage,
};

#[rustfmt::skip]
pub use {
  cluster::Cluster,
  cluster::ClusterCmd,
  node_ring::NodeRing,
  utils::ClusterConfig,
  utils::ClusterEvent,
  utils::ClusterUpdate,
  utils::HBRConfig,
  utils::Member,
};

#[rustfmt::skip]
#[doc(hidden)]
pub use {
  cluster::ClusterMsg,
  cluster::IntraClusterMsg,
  heartbeat_receiver::HeartbeatReceiverMsg,
};
