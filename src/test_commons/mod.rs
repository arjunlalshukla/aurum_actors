use crate as aurum;
use crate::cluster::ClusterEventSimple;
use crate::core::Socket;
use crate::{unify, AurumInterface};
use serde::{Deserialize, Serialize};

#[derive(AurumInterface, Serialize, Deserialize)]
pub enum CoordinatorMsg {
  Event(Socket, ClusterEventSimple),
  TimedOut(Socket, ClusterEventSimple),
  Spawn(u16, Vec<u16>),
  Kill(u16),
  Done,
}

unify!(pub ClusterNodeTypes = ClusterEventSimple | CoordinatorMsg);
