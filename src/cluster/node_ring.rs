use crate::{cluster::Member, core::{Host, Socket}};
use itertools::{Itertools};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use wyhash::{WyHash, wyrng};

pub struct NodeRing {
  pub(in crate::cluster::node_ring) ring: BTreeMap<u64, Arc<Member>>,
  rep_factor: u32,
}
impl NodeRing {
  pub fn new(rep_factor: u32) -> NodeRing {
    NodeRing {
      ring: BTreeMap::new(),
      rep_factor: rep_factor
    }
  }

  pub fn successors(&self, item: &Member) -> Vec<Arc<Member>> {
    let key = hash_code(item);
    self
      .ring
      .range(key..)
      .chain(self.ring.range(..key))
      .map(|(_, x)| x.clone())
      .unique()
      // Skip one to prevent Members from having themselves as successors
      .skip(1)
      .take(self.rep_factor as usize)
      .collect()
  }

  pub fn insert(&mut self, item: Arc<Member>) {
    let mut key = hash_code(&item);
    for _ in 0..item.vnodes {
      self.ring.insert(key, item.clone());
      key = wyrng(&mut key);
    }
  }

  pub fn remove(&mut self, item: &Member) -> Result<(), u32> {
    let mut key = hash_code(&item);
    let mut removed = 0u32;
    for _ in 0..item.vnodes {
      removed += self.ring.remove(&key).is_some() as u32;
      key = wyrng(&mut key);
    }
    if removed == item.vnodes {
      Result::Ok(())
    } else {
      Result::Err(removed)
    }
  }
}

fn hash_code<H: Hash>(item: &H) -> u64 {
  let mut hasher = WyHash::with_seed(0);
  item.hash(&mut hasher);
  hasher.finish()
}


#[test]
fn test_node_ring() {
  let members = (5000u16..5005u16).map(|x| {
    Arc::new(Member {
      socket: Socket::new(Host::DNS("localhost".to_string()), x, 0),
      id: 8,
      vnodes: (x as u32 - 5000) / 2 + 2,
    })
  })
  .collect::<Vec<_>>();

  let mut ring = NodeRing::new(3);
  for m in members.iter() {
    println!("{} -> {}", m.socket.udp, hash_code(m));
    ring.insert(m.clone())
  }
  for (key, member) in ring.ring.iter() {
    println!("{} -> {:?}", key, member);
  }
  let mut expected_successors = Vec::<Vec<u16>>::new();
  expected_successors.push(vec![5001, 5003, 5002]);
  expected_successors.push(vec![5000, 5003, 5002]);
  expected_successors.push(vec![5003, 5004, 5001]);
  expected_successors.push(vec![5002, 5004, 5001]);
  expected_successors.push(vec![5003, 5001, 5000]);
  let successors = members.iter()
    .map(|m| ring.successors(m).iter().map(|x| x.socket.udp).collect())
    .collect::<Vec<Vec<u16>>>();
  assert_eq!(expected_successors, successors);
  for m in members.iter() {
    ring.remove(m).unwrap()
  }
  assert!(ring.ring.is_empty());
}