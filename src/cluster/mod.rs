mod cluster;
mod gossip;
mod heartbeat_receiver;
mod interval_storage;
mod node_ring;
mod utils;

use crate::core::{FailureConfig, FailureMode};

const FAILURE_MODE: FailureMode = FailureMode::None;
const FAILURE_CONFIG: FailureConfig = FailureConfig {
  drop_prob: 0.0,
  delay: None
};

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
  cluster::ClusterMsg,
  cluster::IntraClusterMsg,
  heartbeat_receiver::HBRConfig,
  heartbeat_receiver::HeartbeatReceiverMsg,
  node_ring::NodeRing,
  utils::ClusterConfig,
  utils::ClusterEvent,
  utils::ClusterEventSimple,
  utils::Member,
  utils::UnifiedBounds,
};
