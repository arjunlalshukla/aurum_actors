use crate::core::{
  local_actor_msg_convert, Case, LocalActorMsg, UnifiedBounds,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::hash::Hash;
use std::net::IpAddr;
use std::{fmt::Debug, net::SocketAddr};
use tokio::net::lookup_host;

#[derive(Debug)]
pub enum DeserializeError<U: Debug> {
  IncompatibleInterface(U),
  Other(U),
}

pub fn serialize<T: Serialize + DeserializeOwned>(item: &T) -> Option<Vec<u8>> {
  serde_json::to_vec(item).ok()
}

pub fn deserialize<U, S, I>(
  item: U,
  bytes: &[u8],
) -> Result<LocalActorMsg<S>, DeserializeError<U>>
where
  U: Case<S> + Case<I> + UnifiedBounds,
  S: From<I>,
  I: Serialize + DeserializeOwned,
{
  match serde_json::from_slice::<LocalActorMsg<I>>(bytes) {
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
