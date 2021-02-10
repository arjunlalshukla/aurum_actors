use crate::unify::Case;
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;
use std::net::IpAddr;
use tokio::sync::mpsc::UnboundedSender;

type LocalRef<T> = UnboundedSender<T>;

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum Host { DNS(String), Address(IpAddr) }

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Node { host: Host, udp: u16, tcp: u16 }
impl Node {
  pub fn new(host: Host, udp: u16, tcp: u16) -> Node {
    Node {host: host, udp: udp, tcp: tcp}
  }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct RemoteRef<T> { node: Node, domain: T, name: String }
impl<T> RemoteRef<T> {
  pub fn new(node: Node, domain: T, name: String) -> RemoteRef<T> {
    RemoteRef { node: node, domain: domain, name: name }
  }
}

#[derive(Clone, Debug)]
pub struct ActorRef<Unified, Specific> where 
  Unified: Case<Specific> + Serialize + DeserializeOwned,
  Specific: Serialize + DeserializeOwned {
  remote: RemoteRef<Unified>,
  local: Option<LocalRef<Specific>>
}
impl<Unified, Specific> ActorRef<Unified, Specific> where 
  Unified: Case<Specific> + Serialize + DeserializeOwned,
  Specific: Serialize + DeserializeOwned {
  pub fn new(
    remote: RemoteRef<Unified>,
    local: Option<LocalRef<Specific>>
  ) -> ActorRef<Unified, Specific> {
    ActorRef {remote: remote, local: local}
  }

  pub fn send(msg: Specific) {}
}