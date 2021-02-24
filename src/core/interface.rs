use crate::core::{serialize, Address, Case, DeserializeError, LocalActorMsg};
use itertools::Itertools;
use rand::{self, Rng};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{fmt::Debug, net::SocketAddr};

use super::{ActorName, DatagramHeader, UnifiedBounds, MAX_PACKET_SIZE};

#[derive(Clone)]
pub struct LocalRef<T: Send> {
  pub(crate) func: Arc<dyn Fn(LocalActorMsg<T>) -> bool + Send + Sync>,
}
impl<T: Send> LocalRef<T> {
  pub fn send(&self, item: T) -> bool {
    (&self.func)(LocalActorMsg::Msg(item))
  }

  pub fn eager_kill(&self) -> bool {
    (&self.func)(LocalActorMsg::EagerKill)
  }

  pub fn void() -> LocalRef<T> {
    LocalRef {
      func: Arc::new(|_| false),
    }
  }

  pub fn panic() -> LocalRef<T> {
    LocalRef {
      func: Arc::new(|_| {
        panic!("LocalRef<{}> is a panic", std::any::type_name::<T>())
      }),
    }
  }
}

pub trait HasInterface<T: Serialize + DeserializeOwned> {}

pub trait SpecificInterface<Unified: Debug>
where
  Self: Sized,
{
  fn deserialize_as(
    interface: Unified,
    bytes: &[u8],
  ) -> Result<LocalActorMsg<Self>, DeserializeError<Unified>>;
}

#[derive(Clone, Deserialize, Serialize, Debug)]
#[serde(bound = "Unified: Serialize + DeserializeOwned")]
pub struct Destination<Unified: UnifiedBounds> {
  pub name: ActorName<Unified>,
  pub interface: Unified,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(bound = "Unified: Case<Specific> + Serialize + DeserializeOwned")]
pub struct ActorRef<
  Unified: UnifiedBounds,
  Specific: Send + Serialize + DeserializeOwned,
> {
  addr: Address<Unified>,
  interface: Unified,
  #[serde(skip, default)]
  local: Option<LocalRef<Specific>>,
}
impl<
    Unified: UnifiedBounds + Case<Specific>,
    Specific: Send + Serialize + DeserializeOwned,
  > ActorRef<Unified, Specific>
{
  pub fn new(
    addr: Address<Unified>,
    interface: Unified,
    local: Option<LocalRef<Specific>>,
  ) -> ActorRef<Unified, Specific> {
    ActorRef {
      addr: addr,
      interface: interface,
      local: local,
    }
  }

  pub async fn send(&self, item: Specific) -> Option<bool> {
    match &self.local {
      Some(r) => Some(r.send(item)),
      None => {
        self.remote_send(item).await;
        None
      }
    }
  }

  async fn remote_send(&self, item: Specific) {
    let mut ser = serialize(LocalActorMsg::Msg(item)).unwrap();
    let msg_size = ser.len();
    let dest = Destination {
      name: self.addr.name.clone(),
      interface: self.interface,
    };
    ser.append(&mut serialize(dest.clone()).unwrap());
    let dest_size = ser.len() - msg_size;
    if ser.len() + DatagramHeader::SIZE > MAX_PACKET_SIZE as usize {
      panic!("Serialized item too large");
    }
    let mut buf = vec![0u8; ser.len() + DatagramHeader::SIZE];
    let msg_id = rand::thread_rng().gen::<u64>();
    let header = DatagramHeader {
      msg_id: msg_id,
      seq_num: 0,
      max_seq_num: 0,
      msg_size: msg_size as u32,
      dest_size: dest_size as u16,
    };
    println!("sending {:?} to {:?}", header, dest);
    header.put(&mut buf[..DatagramHeader::SIZE]);
    buf[DatagramHeader::SIZE..DatagramHeader::SIZE + msg_size + dest_size]
      .copy_from_slice(&ser[..]);
    let socks = self.addr.socket.as_udp_addr().await.unwrap();
    let sock: SocketAddr = match socks.iter().exactly_one() {
      Ok(x) => x.clone(),
      Err(_) => panic!("multiple addrs: {:?}", socks),
    };
    let udp = tokio::net::UdpSocket::bind("0.0.0.0:0").await.unwrap();
    udp.send_to(&buf, sock).await.unwrap();
  }
}

impl<
    Unified: UnifiedBounds + Case<Specific>,
    Specific: Send + Serialize + DeserializeOwned,
  > Debug for ActorRef<Unified, Specific>
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("ActorRef")
      .field("Unified", &std::any::type_name::<Unified>())
      .field("Specific", &std::any::type_name::<Specific>())
      .field("addr", &self.addr)
      .field("interface", &self.interface)
      .field("has_local", &self.local.is_some())
      .finish()
  }
}
