use crate as aurum;
use crate::cluster::ClusterEvent;
use crate::core::Socket;
use crate::{unify, AurumInterface};
use serde::{Deserialize, Serialize};

#[derive(AurumInterface, Serialize, Deserialize)]
pub enum CoordinatorMsg {
  Event(Socket, ClusterEvent),
  TimedOut(Socket, ClusterEvent),
  Spawn(u16, Vec<u16>),
  Kill(u16),
}

unify!(pub ClusterNodeTypes = ClusterEvent | CoordinatorMsg);
