#![allow(dead_code)]
use crate::cluster::crdt::{DeltaMutator, CRDT};
#[cfg(test)]
use crate::core::Host;
use crate::core::Socket;
use im::{self, ordmap, OrdMap};
#[cfg(test)]
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::cmp::{max, Ordering};
use std::time::Duration;
#[cfg(test)]
use DeviceMutator::*;

#[derive(
  Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct Device {
  pub socket: Socket,
}

pub enum DeviceMutator {
  Put(Device, DeviceInterval),
  Remove(Device),
}
impl DeltaMutator<Devices> for DeviceMutator {
  fn apply(&self, target: &Devices) -> Devices {
    match self {
      DeviceMutator::Put(dev, interval) => target
        .states
        .get(dev)
        .map(|entry| {
          let mut d = Devices::minimum();
          if entry.interval.filter(|x| x >= interval).is_none() {
            let e = DeviceEntry {
              removals: entry.removals,
              interval: Some(interval.clone()),
            };
            d.states.insert(dev.clone(), e);
          }
          d
        })
        .unwrap_or_else(|| Devices {
          states: ordmap! {
            dev.clone() => DeviceEntry {
              removals: 0,
              interval: Some(interval.clone())
            }
          },
        }),
      DeviceMutator::Remove(dev) => target
        .states
        .get(dev)
        .map(|entry| {
          if entry.interval.is_some() {
            let e = DeviceEntry {
              removals: entry.removals + 1,
              interval: None,
            };
            Devices {
              states: im::OrdMap::unit(dev.clone(), e),
            }
          } else {
            Devices::minimum()
          }
        })
        .unwrap_or_else(|| Devices::minimum()),
    }
  }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct DeviceInterval {
  pub clock: u64,
  pub interval: Duration,
}
impl PartialOrd for DeviceInterval {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(
      self
        .clock
        .cmp(&other.clock)
        .then_with(|| other.interval.cmp(&self.interval)),
    )
  }
}
impl Ord for DeviceInterval {
  fn cmp(&self, other: &Self) -> Ordering {
    self.partial_cmp(other).unwrap()
  }
}

#[derive(
  Copy,
  Clone,
  Debug,
  Eq,
  PartialEq,
  Hash,
  Ord,
  PartialOrd,
  Serialize,
  Deserialize,
)]
pub struct DeviceEntry {
  pub removals: u64,
  pub interval: Option<DeviceInterval>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct Devices {
  pub states: im::OrdMap<Device, DeviceEntry>,
}
impl CRDT for Devices {
  type Delta = DeviceMutator;

  fn delta(&self, changes: &DeviceMutator) -> Self {
    changes.apply(self)
  }

  fn empty(&self) -> bool {
    self.states.is_empty()
  }

  fn join(self, other: Self) -> Self {
    Devices {
      states: self.states.union_with(other.states, max),
    }
  }

  fn minimum() -> Self {
    Devices {
      states: OrdMap::new(),
    }
  }
}

#[cfg(test)]
const E_0_1_100: DeviceEntry = DeviceEntry {
  removals: 0,
  interval: Some(DeviceInterval {
    clock: 1,
    interval: Duration::from_millis(100),
  }),
};

#[cfg(test)]
const E_0_0_50: DeviceEntry = DeviceEntry {
  removals: 0,
  interval: Some(DeviceInterval {
    clock: 0,
    interval: Duration::from_millis(50),
  }),
};

#[cfg(test)]
const E_0_0_100: DeviceEntry = DeviceEntry {
  removals: 0,
  interval: Some(DeviceInterval {
    clock: 0,
    interval: Duration::from_millis(100),
  }),
};

#[cfg(test)]
const E_1_NONE: DeviceEntry = DeviceEntry {
  removals: 1,
  interval: None,
};

#[cfg(test)]
const E_1_0_100: DeviceEntry = DeviceEntry {
  removals: 1,
  interval: Some(DeviceInterval {
    clock: 0,
    interval: Duration::from_millis(100),
  }),
};

#[test]
fn devices_state_join_test() {
  let devices = (0..10)
    .map(|p| Device {
      socket: Socket::new(Host::DNS("localhost".to_string()), p, 0),
    })
    .collect_vec();
  let a = Devices {
    states: ordmap! {
      devices[0].clone() => E_0_1_100,
      devices[1].clone() => E_0_0_50,
      devices[2].clone() => E_0_0_50,
      devices[3].clone() => E_0_0_100,
      devices[4].clone() => E_1_NONE,
      devices[5].clone() => E_0_0_100,
      devices[6].clone() => E_1_NONE,
      devices[7].clone() => E_1_0_100,
      devices[8].clone() => E_1_0_100
    },
  };
  let b = Devices {
    states: ordmap! {
      devices[0].clone() => E_0_0_50,
      devices[1].clone() => E_0_1_100,
      devices[2].clone() => E_0_0_100,
      devices[3].clone() => E_0_0_50,
      devices[4].clone() => E_0_0_100,
      devices[5].clone() => E_1_NONE,
      devices[6].clone() => E_1_0_100,
      devices[7].clone() => E_1_NONE,
      devices[9].clone() => E_1_0_100
    },
  };
  let c = Devices {
    states: ordmap! {
      devices[0].clone() => E_0_1_100,
      devices[1].clone() => E_0_1_100,
      devices[2].clone() => E_0_0_50,
      devices[3].clone() => E_0_0_50,
      devices[4].clone() => E_1_NONE,
      devices[5].clone() => E_1_NONE,
      devices[6].clone() => E_1_0_100,
      devices[7].clone() => E_1_0_100,
      devices[8].clone() => E_1_0_100,
      devices[9].clone() => E_1_0_100
    },
  };
  let j = a.join(b);
  if j != c {
    println!("Expected:");
    for (dev, int) in c.states.iter() {
      println!("{} -> {:?}", dev.socket.udp, int);
    }
    println!("Actual:");
    for (dev, int) in j.states.iter() {
      println!("{} -> {:?}", dev.socket.udp, int);
    }
    assert!(false);
  }
}

#[test]
fn devices_state_deltas_test() {
  let devices = (0..11)
    .map(|p| Device {
      socket: Socket::new(Host::DNS("localhost".to_string()), p, 0),
    })
    .collect_vec();
  let a = Devices {
    states: ordmap! {
      devices[0].clone() => E_1_NONE,
      devices[1].clone() => E_0_0_50,
      devices[2].clone() => E_0_0_50,
      devices[3].clone() => E_0_0_100,
      devices[4].clone() => E_1_NONE,
      devices[5].clone() => E_0_0_100,
      devices[6].clone() => E_1_NONE,
      devices[7].clone() => E_0_0_50,
      devices[8].clone() => E_0_0_50
    },
  };
  let c = Devices {
    states: ordmap! {
      devices[0].clone() => E_1_NONE,
      devices[1].clone() => E_0_1_100,
      devices[2].clone() => E_0_0_50,
      devices[3].clone() => E_0_0_50,
      devices[4].clone() => E_1_NONE,
      devices[5].clone() => E_1_NONE,
      devices[6].clone() => E_1_0_100,
      devices[7].clone() => E_0_0_50,
      devices[8].clone() => E_0_0_50,
      devices[9].clone() => E_0_0_100
    },
  };
  let mutations = vec![
    Put(devices[1].clone(), E_0_1_100.interval.unwrap()),
    Put(devices[3].clone(), E_0_0_50.interval.unwrap()),
    Remove(devices[4].clone()),
    Remove(devices[5].clone()),
    Put(devices[6].clone(), E_1_0_100.interval.unwrap()),
    Put(devices[7].clone(), E_0_0_50.interval.unwrap()),
    Put(devices[8].clone(), E_0_0_100.interval.unwrap()),
    Put(devices[9].clone(), E_0_0_100.interval.unwrap()),
    Remove(devices[10].clone()),
  ];
  let exp_deltas = vec![
    Devices {
      states: OrdMap::unit(devices[1].clone(), E_0_1_100),
    },
    Devices {
      states: OrdMap::unit(devices[3].clone(), E_0_0_50),
    },
    Devices::minimum(),
    Devices {
      states: OrdMap::unit(devices[5].clone(), E_1_NONE),
    },
    Devices {
      states: OrdMap::unit(devices[6].clone(), E_1_0_100),
    },
    Devices::minimum(),
    Devices::minimum(),
    Devices {
      states: OrdMap::unit(devices[9].clone(), E_0_0_100),
    },
    Devices::minimum(),
  ];
  let deltas = mutations.into_iter().map(|m| m.apply(&a)).collect_vec();
  if deltas != exp_deltas {
    for (i, (d, e)) in deltas.iter().zip(exp_deltas.iter()).enumerate() {
      if d != e {
        println!("Index: {}", i);
        println!("Expected:");
        for (dev, int) in e.states.iter() {
          println!("{} -> {:?}", dev.socket.udp, int);
        }
        println!("Actual:");
        for (dev, int) in d.states.iter() {
          println!("{} -> {:?}", dev.socket.udp, int);
        }
      }
    }
    assert!(false);
  }
  let j = deltas.into_iter().fold(Devices::minimum(), Devices::join);
  assert_eq!(j.join(a), c);
}
