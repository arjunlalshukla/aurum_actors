use crate::core::{
  local_actor_msg_convert, ActorName, Case, LocalActorMsg, UnifiedBounds,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::hash::Hash;
use std::net::IpAddr;
use std::{fmt::Debug, net::SocketAddr};
use tokio::net::lookup_host;

pub(crate) const MAX_PACKET_SIZE: usize = 65507;

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
  bytes: &[u8],
) -> Result<LocalActorMsg<Specific>, DeserializeError<Unified>>
where
  Unified: Case<Specific> + Case<Interface> + UnifiedBounds,
  Specific: From<Interface>,
  Interface: Serialize + DeserializeOwned,
{
  match serde_json::from_slice::<LocalActorMsg<Interface>>(bytes) {
    Ok(res) => Result::Ok(local_actor_msg_convert(res)),
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
  pub host: Host,
  pub udp: u16,
  pub tcp: u16,
}
impl Socket {
  pub fn new(host: Host, udp: u16, tcp: u16) -> Socket {
    Socket {
      host: host,
      udp: udp,
      tcp: tcp,
    }
  }

  pub async fn as_udp_addr(&self) -> std::io::Result<Vec<SocketAddr>> {
    match &self.host {
      Host::IP(ip) => Ok(vec![SocketAddr::new(*ip, self.udp)]),
      Host::DNS(s) => lookup_host((s.as_str(), self.udp))
        .await
        .map(|x| x.collect()),
    }
  }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(bound = "Unified: UnifiedBounds")]
pub struct Address<Unified: UnifiedBounds> {
  pub socket: Socket,
  pub name: ActorName<Unified>,
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
