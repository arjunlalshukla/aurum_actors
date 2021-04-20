use crate::core::{FailureConfig, Socket};
use im::HashMap;

#[derive(Clone)]
pub struct FailureConfigMap {
  pub cluster_wide: FailureConfig,
  pub node_wide: HashMap<Socket, FailureConfig>
}
impl FailureConfigMap {
  pub fn get(&self, socket: &Socket) -> &FailureConfig {
    self.node_wide.get(socket).unwrap_or(&self.cluster_wide)
  }
}
