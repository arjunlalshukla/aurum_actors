use crate::cluster::Member;
use crdts::VClock;
use im;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub struct Gossip {
  vclk: VClock<Member>,
  states: BTreeMap<Member, MachineState>,
}

#[derive(
  Serialize, Deserialize, Hash, PartialEq, Eq, Ord, PartialOrd, Clone, Copy,
)]
pub enum MachineState {
  Joined,
  Up,
  Leaving,
  Down,
  Removed,
}
