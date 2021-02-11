use std::marker::PhantomData;
use std::sync::Arc;
use std::net::IpAddr;
use crate::unify::Case;
use crossbeam::{channel::Sender, thread::ScopedThreadBuilder};
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;

type LocalRef<T> = Arc<dyn Fn(T) -> bool>;

trait Actor<Unified: Case<Msg>, Msg> {
  fn pre_start(&mut self) {}
  fn recv(&mut self, ctx: ActorContext<Unified, Msg>, msg: Msg);
  fn post_stop(&mut self) {}
}

pub struct ActorContext<Unified, Specific> {
  tx: Sender<Specific>,
  p: PhantomData<Unified>
}
impl<Unified, Specific> ActorContext<Unified, Specific> {
  fn local_ref<T>(&self) -> LocalRef<T>
   where Specific: From<T> + 'static {
    let sender = self.tx.clone();
    Arc::new(move |x: T| sender.send(Specific::from(x)).is_ok())
  }
}

pub enum DeserializeError<Unified> {
  IncompatibleInterface(Unified),
  Other(Unified)
}

pub trait SpecificInterface<Unified> where 
 Self: Serialize + DeserializeOwned + Sized {
  fn deserialize_as(interface: Unified, bytes: Vec<u8>) -> 
    Result<Self, DeserializeError<Unified>>;
}

pub fn serialize<T>(item: T) -> Option<Vec<u8>>
 where T: Serialize + DeserializeOwned {
  serde_json::to_vec(&item).ok()
}

pub fn deserialize<T>(bytes: Vec<u8>) -> Option<T>
 where T: Serialize + DeserializeOwned {
  serde_json::from_slice(bytes.as_slice()).ok()
}


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