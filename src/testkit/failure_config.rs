use crate::core::Socket;
use im;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Dictates how messages should be dropped.
pub enum FailureMode {
  /// Individual packets are either sent/dropped/delayed by individually generated amounts.
  Packet,
  /// Messages are either sent/dropped/delayed in full. This is no different from [`Packet`] for
  /// single-packet messages.
  Message,
  /// Messages will not be dropped.
  None,
}

#[derive(Default, Serialize, Deserialize, Clone, Copy, Debug)]
pub struct FailureConfig {
  pub drop_prob: f64,
  pub delay: Option<(Duration, Duration)>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct FailureConfigMap {
  pub cluster_wide: FailureConfig,
  pub node_wide: im::HashMap<Socket, FailureConfig>,
}
impl FailureConfigMap {
  pub fn get(&self, socket: &Socket) -> &FailureConfig {
    self.node_wide.get(socket).unwrap_or(&self.cluster_wide)
  }
}
