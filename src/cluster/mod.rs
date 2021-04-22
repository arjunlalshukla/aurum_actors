use crate::testkit::{FailureMode, LogLevel};
mod cluster;

mod gossip;
mod heartbeat_receiver;
mod interval_storage;
mod node_ring;
mod utils;

pub const FAILURE_MODE: FailureMode = FailureMode::Message;
pub const LOG_LEVEL: LogLevel = LogLevel::Info;

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
  heartbeat_receiver::HeartbeatReceiverMsg,
  node_ring::NodeRing,
  utils::ClusterConfig,
  utils::ClusterEvent,
  utils::ClusterEventSimple,
  utils::HBRConfig,
  utils::Member,
  utils::UnifiedBounds,
};
