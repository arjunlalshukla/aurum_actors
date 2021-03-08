#![allow(unused_imports, dead_code, unused_variables)]

use crate as aurum;
use crate::core::{Actor, ActorContext, ActorRef, Case, LocalRef, Node, Socket};
use async_trait::async_trait;
use aurum_macros::AurumInterface;
use serde::{Deserialize, Serialize};

trait UnifiedBounds:
  crate::core::UnifiedBounds
  + Case<ClusterMsg<Self>>
  + Case<IntraClusterMsg<Self>>
{
}
impl<T> UnifiedBounds for T where
  T: crate::core::UnifiedBounds
    + Case<ClusterMsg<Self>>
    + Case<IntraClusterMsg<Self>>
{
}

#[derive(AurumInterface)]
#[aurum(local)]
enum ClusterMsg<U: UnifiedBounds> {
  #[aurum]
  IntraMsg(IntraClusterMsg<U>),
  #[aurum(local)]
  LocalCmd(ClusterCmd),
}

#[derive(Serialize, Deserialize)]
#[serde(bound = "U: UnifiedBounds")]
enum IntraClusterMsg<U: UnifiedBounds> {
  Join(ActorRef<U, IntraClusterMsg<U>>),
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
  seeds: Vec<ActorRef<U, IntraClusterMsg<U>>>,
  subscribers: Vec<LocalRef<ClusterEvent>>,
}
#[async_trait]
impl<U: UnifiedBounds> Actor<U, ClusterMsg<U>> for Cluster<U> {
  async fn recv(
    &mut self,
    ctx: &ActorContext<U, ClusterMsg<U>>,
    msg: ClusterMsg<U>,
  ) {
    let cmd = ctx.local_interface::<ClusterCmd>();
    let intra = ctx.interface::<IntraClusterMsg<U>>();
    match msg {
      _ => {}
    }
  }
}
impl<U: UnifiedBounds> Cluster<U> {
  pub fn new(node: &Node<U>) {
    
  }
}