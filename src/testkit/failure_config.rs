use crate::core::Socket;
use im::HashMap;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub enum FailureMode {
  Packet,
  Message,
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
  pub node_wide: HashMap<Socket, FailureConfig>,
}
impl FailureConfigMap {
  pub fn get(&self, socket: &Socket) -> &FailureConfig {
    self.node_wide.get(socket).unwrap_or(&self.cluster_wide)
  }
}
