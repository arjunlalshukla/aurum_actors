use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::sync::Arc;
use Ordering::*;

use crate::cluster::Member;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Gossip {
  states: BTreeMap<Arc<Member>, MachineState>,
}
impl Gossip {
  pub fn merge(&self, other: Gossip) -> Vec<(Arc<Member>, MachineState)> {
    let mut v = Vec::new();
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
                  v.push((l_mem.clone(), r_state));
                }
                Equal => {}
                Greater => {}
              }
              left = left_iter.next();
              right = right_iter.next();
            }
            Greater => {
              v.push((r_mem, r_state));
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
          v.push((r_mem, r_state));
          left = None;
          right = right_iter.next()
        }
        _ => break,
      }
    }
    v
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
  Joined,
  Up,
  Leaving,
  Down,
  Removed,
}

#[cfg(test)]
use crate::core::{Host, Socket};
#[cfg(test)]
use maplit::btreemap;
#[cfg(test)]
use MachineState::*;

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
  let local = Gossip {
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
      members[8].clone() => Joined,
    },
  };
  let changes = local
    .merge(recvd)
    .into_iter()
    .map(|(m, s)| (m.socket.udp, s))
    .collect::<Vec<_>>();
  let expected = vec![
    (members[1].socket.udp, Up),
    (members[3].socket.udp, Up),
    (members[7].socket.udp, Down),
    (members[8].socket.udp, Joined),
  ];
  assert_eq!(expected, changes);
}
