use crate::core::{ActorName, Case, UnifiedBounds};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::hash::Hash;
use std::net::IpAddr;

#[derive(Debug)]
pub enum DeserializeError<Unified: Debug> {
  IncompatibleInterface(Unified),
  Other(Unified),
}

pub fn serialize<T>(item: T) -> Option<Vec<u8>>
where
  T: Serialize + DeserializeOwned,
{
  serde_json::to_vec(&item).ok()
}

pub fn deserialize<Unified, Specific, Interface>(
  item: Unified,
  bytes: Vec<u8>,
) -> Result<Specific, DeserializeError<Unified>>
where
  Unified: Case<Specific> + Case<Interface> + UnifiedBounds,
  Specific: From<Interface>,
  Interface: Serialize + DeserializeOwned,
{
  match serde_json::from_slice::<Interface>(bytes.as_slice()) {
    Ok(res) => Result::Ok(Specific::from(res)),
    Err(_) => Result::Err(DeserializeError::Other(item)),
  }
}
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum Host {
  DNS(String),
  IP(IpAddr),
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Socket {
  host: Host,
  udp: u16,
  tcp: u16,
}
impl Socket {
  pub fn new(host: Host, udp: u16, tcp: u16) -> Socket {
    Socket {
      host: host,
      udp: udp,
      tcp: tcp,
    }
  }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(bound = "Unified: UnifiedBounds")]
pub struct Address<Unified: UnifiedBounds> {
  socket: Socket,
  name: ActorName<Unified>,
}
impl<Unified: UnifiedBounds> Address<Unified> {
  pub fn new<Specific>(
    sock: Socket,
    name: ActorName<Unified>,
  ) -> Address<Unified>
  where
    Unified: Case<Specific>,
  {
    Address {
      socket: sock,
      name: name,
    }
  }
}
