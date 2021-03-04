#![allow(unused_imports, dead_code, unused_variables)]

use crate as aurum;
use crate::core::{Actor, ActorContext, ActorRef, Case, LocalRef, Socket};
use async_trait::async_trait;
use aurum_macros::AurumInterface;
use serde::{Deserialize, Serialize};

trait UnifiedBounds:
  crate::core::UnifiedBounds + Case<ClusterMsg> + Case<IntraClusterMsg>
{
}
impl<T> UnifiedBounds for T where
  T: crate::core::UnifiedBounds + Case<ClusterMsg> + Case<IntraClusterMsg>
{
}

#[derive(AurumInterface)]
#[aurum(local)]
enum ClusterMsg {
  #[aurum]
  IntraMsg(IntraClusterMsg),
  #[aurum(local)]
  LocalCmd(ClusterCmd),
}

#[derive(Serialize, Deserialize)]
enum IntraClusterMsg {
  Heartbeat,
}

enum ClusterCmd {
  Join(Vec<Socket>),
  Leave,
  Subscribe(LocalRef<ClusterEvent>, Vec<ClusterEventType>),
}

enum ClusterEventType {}

enum ClusterEvent {}

struct Cluster<U: UnifiedBounds> {
  members: Vec<ActorRef<U, IntraClusterMsg>>,
  subscribers: Vec<LocalRef<ClusterEvent>>,
}

#[async_trait]
impl<U: UnifiedBounds> Actor<U, ClusterMsg> for Cluster<U> {
  async fn recv(&mut self, _: &ActorContext<U, ClusterMsg>, msg: ClusterMsg) {
    match msg {
      ClusterMsg::IntraMsg(IntraClusterMsg::Heartbeat) => {}
      ClusterMsg::LocalCmd(ClusterCmd::Subscribe(s, _)) => {
        self.subscribers.push(s)
      }
      ClusterMsg::LocalCmd(ClusterCmd::Leave) => {}
      ClusterMsg::LocalCmd(ClusterCmd::Join(sockets)) => {}
    }
  }
}
