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
  utils::ClusterEventSimple,
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