use std::fmt::Debug;
use std::net::IpAddr;
use crate::core::Case;
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;

#[derive(Debug)]
pub enum DeserializeError<Unified: Debug> {
  IncompatibleInterface(Unified),
  Other(Unified)
}

pub fn serialize<T>(item: T) -> Option<Vec<u8>>
 where T: Serialize + DeserializeOwned {
  serde_json::to_vec(&item).ok()
}

pub fn deserialize<Unified, Specific, Interface>(item: Unified, bytes: Vec<u8>) ->
 Result<Specific, DeserializeError<Unified>> where 
 Unified: Case<Specific> + Case<Interface> + Debug, Specific: From<Interface>,
 Interface: Serialize + DeserializeOwned {
  match serde_json::from_slice::<Interface>(bytes.as_slice()) {
    Ok(res) => Result::Ok(Specific::from(res)),
    Err(_) => Result::Err(DeserializeError::Other(item))
  }
}
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum Host { DNS(String), IP(IpAddr) }

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Socket { host: Host, udp: u16, tcp: u16 }
impl Socket {
  pub fn new(host: Host, udp: u16, tcp: u16) -> Socket {
    Socket {host: host, udp: udp, tcp: tcp}
  }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Address<T: Clone> { node: Socket, recv_type: T, name: String }
impl<T> Address<T> where T: Clone {
  pub fn new(node: Socket, recv_type: T, name: String) -> Address<T> {
    Address { node: node, recv_type: recv_type, name: name }
  }
}