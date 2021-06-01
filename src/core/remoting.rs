use crate::core::{ActorName, Case, SpecificInterface, UnifiedType};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::hash::Hash;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::{cmp::PartialEq, marker::PhantomData};
use std::{fmt, str::FromStr};
use tokio::net::lookup_host;

#[derive(
  Clone, Debug, Deserialize, Eq, Hash, PartialEq, Ord, PartialOrd, Serialize,
)]
pub enum Host {
  DNS(String),
  IP(IpAddr),
}
impl From<String> for Host {
  fn from(s: String) -> Self {
    match IpAddr::from_str(s.as_str()) {
      Ok(ip) => Host::IP(ip),
      Err(_) => Host::DNS(s),
    }
  }
}

#[derive(
  Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, Ord, PartialOrd,
)]
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
        .map(|x| x.filter(|a| a.is_ipv4()).collect()),
    }
  }
}
impl fmt::Display for Socket {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match &self.host {
      Host::DNS(s) => write!(f, "DNS({}):{}|{}", s, self.udp, self.tcp),
      Host::IP(ip) => write!(f, "IP({}):{}|{}", ip, self.udp, self.tcp),
    }
  }
}
impl Default for Socket {
  fn default() -> Self {
    Self {
      host: Host::IP(IpAddr::V4(Ipv4Addr::UNSPECIFIED)),
      udp: 0,
      tcp: 0,
    }
  }
}

#[derive(Eq, PartialEq, Clone, Hash, Debug, Deserialize, Serialize)]
#[serde(bound = "U: Serialize + DeserializeOwned")]
pub struct DestinationUntyped<U: UnifiedType> {
  pub name: ActorName<U>,
  pub interface: U,
}

#[derive(Eq, Deserialize, Serialize)]
#[serde(bound = "U: Serialize + DeserializeOwned")]
pub struct Destination<U: UnifiedType + Case<I>, I> {
  pub untyped: DestinationUntyped<U>,
  #[serde(skip)]
  pub x: PhantomData<I>,
}
impl<U: UnifiedType + Case<I>, I> Destination<U, I> {
  pub fn new<S>(s: String) -> Destination<U, I>
  where
    U: Case<S>,
    S: From<I> + SpecificInterface<U>,
  {
    Destination {
      untyped: DestinationUntyped {
        name: ActorName::new::<S>(s),
        interface: <U as Case<I>>::VARIANT,
      },
      x: PhantomData,
    }
  }

  pub fn name(&self) -> &ActorName<U> {
    &self.untyped.name
  }

  pub fn untyped(&self) -> &DestinationUntyped<U> {
    &self.untyped
  }

  pub fn valid(&self) -> bool {
    self
      .untyped
      .name
      .recv_type
      .has_interface(<U as Case<I>>::VARIANT)
  }
}
impl<U: UnifiedType + Case<I>, I> Clone for Destination<U, I> {
  fn clone(&self) -> Self {
    Destination {
      untyped: self.untyped.clone(),
      x: PhantomData,
    }
  }
}
impl<U: UnifiedType + Case<I>, I> PartialEq for Destination<U, I> {
  fn eq(&self, other: &Self) -> bool {
    self.untyped == other.untyped
  }
}
impl<U: UnifiedType + Case<I>, I> Hash for Destination<U, I> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.untyped.hash(state);
  }
}
impl<U: UnifiedType + Case<I>, I> Debug for Destination<U, I> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Destination")
      .field("Unified", &std::any::type_name::<U>())
      .field("Interface", &std::any::type_name::<I>())
      .field("untyped", &self.untyped)
      .finish()
  }
}
