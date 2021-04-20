use crate::cluster::{ClusterEvent, Member};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::Arc;

use MachineState::*;
use Ordering::*;

#[derive(Serialize, Deserialize, Clone)]
pub struct Gossip {
  pub states: BTreeMap<Arc<Member>, MachineState>,

}
impl Gossip {
  pub fn merge(&mut self, other: Gossip) -> Vec<ClusterEvent> {
    let mut events = Vec::new();
    let mut changes = Vec::new();
    let mut left_iter = self.states.iter();
    let mut right_iter = other.states.into_iter();
    let mut left = left_iter.next();
    let mut right = right_iter.next();
    loop {
      match (left, right) {
        (Some((l_mem, l_state)), Some((r_mem, r_state))) => {
          match l_mem.cmp(&r_mem) {
            Equal => {
              match l_state.cmp(&r_state) {
                Less => {
                  let event = match &r_state {
                    Up => ClusterEvent::Added(r_mem.clone()),
                    _ => ClusterEvent::Removed(r_mem.clone()),
                  };
                  changes.push((r_mem.clone(), r_state));
                  events.push(event);
                }
                Equal => {}
                Greater => {}
              }
              left = left_iter.next();
              right = right_iter.next();
            }
            Greater => {
              let event = match &r_state {
                Up => ClusterEvent::Added(r_mem.clone()),
                _ => ClusterEvent::Removed(r_mem.clone()),
              };
              changes.push((r_mem.clone(), r_state));
              events.push(event);
              left = Some((l_mem, l_state));
              right = right_iter.next();
            }
            Less => {
              left = left_iter.next();
              right = Some((r_mem, r_state));
            }
          }
        }
        (None, Some((r_mem, r_state))) => {
          let event = match &r_state {
            Up => ClusterEvent::Added(r_mem.clone()),
            _ => ClusterEvent::Removed(r_mem.clone()),
          };
          changes.push((r_mem.clone(), r_state));
          events.push(event);
          left = None;
          right = right_iter.next()
        }
        _ => break,
      }
    }
    for (member, state) in changes {
      self.states.insert(member, state);
    }
    events
  }
}

#[derive(
  Serialize,
  Deserialize,
  Hash,
  PartialEq,
  Eq,
  Ord,
  PartialOrd,
  Clone,
  Copy,
  Debug,
)]
pub enum MachineState {
  Up,
  Down,
}

#[cfg(test)]
use crate::core::{Host, Socket};
#[cfg(test)]
use maplit::btreemap;

#[test]
fn test_gossip_merge() {
  let members = (5000u16..5009u16)
    .map(|x| {
      Arc::new(Member {
        socket: Socket::new(Host::DNS("localhost".to_string()), x, 0),
        id: x as u64,
        vnodes: (x as u32 - 5000) / 2 + 2,
      })
    })
    .collect::<Vec<_>>();
  let mut local = Gossip {
    states: btreemap! {
      members[0].clone() => Up,
      members[2].clone() => Up,
      members[4].clone() => Up,
      members[5].clone() => Up,
      members[6].clone() => Down,
      members[7].clone() => Up,
    },
  };
  let recvd = Gossip {
    states: btreemap! {
      members[1].clone() => Up,
      members[2].clone() => Up,
      members[3].clone() => Up,
      members[5].clone() => Up,
      members[6].clone() => Up,
      members[7].clone() => Down,
      members[8].clone() => Up,
    },
  };
  let changes = local.merge(recvd);
  let expected_local = btreemap! {
    members[0].clone() => Up,
    members[1].clone() => Up,
    members[2].clone() => Up,
    members[3].clone() => Up,
    members[4].clone() => Up,
    members[5].clone() => Up,
    members[6].clone() => Down,
    members[7].clone() => Down,
    members[8].clone() => Up,
  };
  assert_eq!(local.states, expected_local);
  let expected_changes = vec![
    ClusterEvent::Added(members[1].clone()),
    ClusterEvent::Added(members[3].clone()),
    ClusterEvent::Removed(members[7].clone()),
    ClusterEvent::Added(members[8].clone()),
  ];
  assert_eq!(changes, expected_changes);
}
