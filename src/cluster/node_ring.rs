use crate::{
  cluster::Member,
  core::{Host, Socket},
};
use itertools::Itertools;
use linked_hash_map::LinkedHashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::{collections::BTreeMap, mem};
use wyhash::{wyrng, WyHash};

pub struct NodeRing {
  pub(in crate::cluster::node_ring) ring: BTreeMap<u64, (bool, Arc<Member>)>,
  rep_factor: usize,
}
impl NodeRing {
  pub fn new(rep_factor: usize) -> NodeRing {
    NodeRing {
      ring: BTreeMap::new(),
      rep_factor: rep_factor,
    }
  }

  // This function will include the member itself if members are being hashed.
  // In this situation, we will need to skip one.
  pub fn managers<H: Hash>(&self, item: &H, num: usize) -> Vec<Arc<Member>> {
    let key = hash_code(item);
    self
      .ring
      .range(key..)
      .chain(self.ring.range(..key))
      .map(|(_, (_, x))| x.clone())
      .unique()
      .take(num)
      .collect()
  }

  pub fn node_managers(&self, member: &Member) -> Vec<Arc<Member>> {
    self
      .managers(member, self.rep_factor + 1)
      .into_iter()
      .skip(1)
      .collect()
  }

  pub fn charges(&self, member: &Member) -> Vec<Arc<Member>> {
    let keys = {
      let mut key = hash_code(&member);
      let mut v = Vec::with_capacity(member.vnodes as usize);
      for _ in 0..member.vnodes {
        v.push(key);
        key = wyrng(&mut key);
      }
      v
    };
    let mut c = Vec::new();
    keys.into_iter().for_each(|key| {
      let mut uniq = LinkedHashMap::<&Member, (&Arc<Member>, bool)>::new();
      let span = self
        .ring
        .range(..key)
        .rev()
        .chain(self.ring.range(key..).rev())
        .take_while(|(_, (_, x))| &**x != member);
      let mut i = 0;
      for (_, (first, mbr)) in span {
        match uniq.entry(mbr) {
          linked_hash_map::Entry::Occupied(mut o) => {
            o.insert((o.get().0, *first || (*o.get()).1));
          }
          linked_hash_map::Entry::Vacant(v) => {
            if i == self.rep_factor {
              break;
            }
            i += 1;
            v.insert((mbr, *first));
          }
        }
      }
      uniq
        .iter()
        .filter(|(_, (_, b))| *b)
        .for_each(|(_, (mbr, _))| c.push((*mbr).clone()))
    });
    c
  }

  pub fn insert(&mut self, item: Arc<Member>) {
    let mut key = hash_code(&item);
    self.ring.insert(key, (true, item.clone()));
    for _ in 1..item.vnodes {
      key = wyrng(&mut key);
      self.ring.insert(key, (false, item.clone()));
    }
  }

  pub fn remove(&mut self, item: &Member) -> Result<(), u32> {
    let mut key = hash_code(&item);
    let mut removed = 0u32;
    for _ in 0..std::cmp::max(1, item.vnodes) {
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
  let members = (5000u16..5005u16)
    .map(|x| {
      Arc::new(Member {
        socket: Socket::new(Host::DNS("localhost".to_string()), x, 0),
        id: 8,
        vnodes: (x as u32 - 5000) / 2 + 2,
      })
    })
    .collect::<Vec<_>>();
  let mut ring = NodeRing::new(3);
  for m in members.iter() {
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
  let successors = members
    .iter()
    .map(|m| ring.node_managers(m).iter().map(|x| x.socket.udp).collect())
    .collect::<Vec<Vec<u16>>>();
  assert_eq!(expected_successors, successors);
  drop(expected_successors);
  drop(successors);
  let mut expected_charges = Vec::<Vec<u16>>::new();
  expected_charges.push(vec![5001, 5004]);
  expected_charges.push(vec![5004, 5002, 5003, 5000]);
  expected_charges.push(vec![5003, 5000, 5001]);
  expected_charges.push(vec![5000, 5001, 5004, 5002]);
  expected_charges.push(vec![5002, 5003]);
  let charges = members
    .iter()
    .map(|m| ring.charges(m).iter().map(|x| x.socket.udp).collect())
    .collect::<Vec<Vec<u16>>>();
  assert_eq!(expected_charges, charges);
  for m in members.iter() {
    ring.remove(m).unwrap()
  }
  assert!(ring.ring.is_empty());
}
