use crate as aurum;
use crate::cluster::{ClusterConfig, ClusterEvent, ClusterEventSimple, HBRConfig};
use crate::core::{ActorRef, Socket};
use crate::testkit::FailureConfigMap;
use crate::{unify, AurumInterface};
use serde::{Deserialize, Serialize};

#[derive(AurumInterface, Serialize, Deserialize)]
pub enum CoordinatorMsg {
  Event(Socket, ClusterEventSimple),
  TimedOut(Socket, ClusterEventSimple),
  Spawn(u16, Vec<u16>),
  Kill(u16),
  Up(ActorRef<ClusterNodeTypes, ClusterNodeMsg>),
  Done,
}

#[derive(AurumInterface, Serialize, Deserialize)]
pub enum ClusterNodeMsg {
  #[aurum(local)]
  Event(ClusterEvent),
  FailureMap(FailureConfigMap, ClusterConfig, HBRConfig),
}

unify!(pub ClusterNodeTypes = ClusterNodeMsg | CoordinatorMsg);
