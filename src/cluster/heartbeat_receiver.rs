#![allow(unused_imports, dead_code, unused_variables)]

use crate as aurum;
use crate::cluster::{ClusterMsg, IntervalStorage, Member, UnifiedBounds};
use crate::core::{ActorContext, Case, LocalRef, TimeoutActor};
use crate::AurumInterface;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

#[derive(AurumInterface, Serialize, Deserialize)]
enum HeartbeatReceiverMsg {
  Heartbeat(Duration, u32),
}

enum HBRState {
  Initial,
  Receiving(IntervalStorage, Duration, u32)
}

struct HeartbeatReceiver<U>
where
  U: UnifiedBounds + Case<HeartbeatReceiverMsg>,
{
  detector: IntervalStorage,
  supervisor: LocalRef<ClusterMsg<U>>,
  member: Arc<Member>,
  charge: Arc<Member>,
  phi: f64,
  interval: Duration,
  count: u32,
}
#[async_trait]
impl<U> TimeoutActor<U, HeartbeatReceiverMsg> for HeartbeatReceiver<U>
where
  U: UnifiedBounds + Case<HeartbeatReceiverMsg>,
{
  async fn recv(
    &mut self,
    _: &ActorContext<U, HeartbeatReceiverMsg>,
    _: HeartbeatReceiverMsg,
  ) -> Option<Duration> {
    None
  }

  async fn timeout(
    &mut self,
    _: &crate::core::ActorContext<U, HeartbeatReceiverMsg>,
  ) -> Option<Duration> {
    None
  }
}
