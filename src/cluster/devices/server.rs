#![allow(dead_code, unused_imports)]
use crate as aurum;
use crate::cluster::{ClusterEvent, NodeRing};
use crate::core::Socket;
use crate::AurumInterface;
use serde::{Deserialize, Serialize};

#[derive(AurumInterface, Serialize, Deserialize)]
#[aurum]
enum ExSvrMsg {
  Ring(NodeRing),
  Event(ClusterEvent),
}

#[derive(Serialize, Deserialize)]
enum RemoteSvrMsg {}
