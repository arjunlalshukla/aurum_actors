use crate::core::{ActorId, Case, RootMessage, UnifiedType};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::hash::Hash;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::{cmp::PartialEq, marker::PhantomData};
use std::{fmt, str::FromStr};
use tokio::net::lookup_host;

/// The DNS name or IP address of the machine hosting a [`Node`](crate::core::Node).
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Ord, PartialOrd, Serialize)]
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

/// The remote address of a [`Node`](crate::core::Node), reachable by remoting.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, Ord, PartialOrd)]
pub struct Socket {
  /// The DNS name or IP address of the machine hosting the [`Node`](crate::core::Node).
  pub host: Host,
  /// The UDP port our [`Node`](crate::core::Node) receives on.
  pub udp: u16,
  /// The TCP port our [`Node`](crate::core::Node) receives on.
  pub tcp: u16,
}
impl Socket {
  /// Creates a new [`Socket`]
  pub fn new(host: Host, udp: u16, tcp: u16) -> Socket {
    Socket {
      host: host,
      udp: udp,
      tcp: tcp,
    }
  }

  /// Uses this the UDP port of this [`Socket`] in a raw [`SocketAddr`]. If the [`Host`] for this
  /// [`Socket`] is a DNS name, this funtion will perform a DNS lookup. Only returns an error if the
  /// DNS lookup fails.
  pub async fn as_udp_addr(&self) -> std::io::Result<Vec<SocketAddr>> {
    match &self.host {
      Host::IP(ip) => Ok(vec![SocketAddr::new(*ip, self.udp)]),
      Host::DNS(s) => {
        lookup_host((s.as_str(), self.udp)).await.map(|x| x.filter(|a| a.is_ipv4()).collect())
      }
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
pub(in crate::core) struct DestinationUntyped<U: UnifiedType> {
  pub name: ActorId<U>,
  pub interface: U,
}

/// The local part of an actor's messaging address.
#[derive(Eq, Deserialize, Serialize)]
#[serde(bound = "U: Serialize + DeserializeOwned")]
pub struct Destination<U: UnifiedType + Case<I>, I> {
  untyped: DestinationUntyped<U>,
  #[serde(skip)]
  x: PhantomData<I>,
}
impl<U: UnifiedType + Case<I>, I> Destination<U, I> {
  /// Forges a new [`Destination`]
  pub fn new<S>(s: String) -> Destination<U, I>
  where
    U: Case<S>,
    S: From<I> + RootMessage<U>,
  {
    Destination {
      untyped: DestinationUntyped {
        name: ActorId::new::<S>(s),
        interface: <U as Case<I>>::VARIANT,
      },
      x: PhantomData,
    }
  }

  /// Returns the [`ActorId`] component of this [`Destination`]
  pub fn name(&self) -> &ActorId<U> {
    &self.untyped.name
  }

  pub(in crate::core) fn untyped(&self) -> &DestinationUntyped<U> {
    &self.untyped
  }

  /// Tests whether the [`ActorId`] held by this [`Destination`] can receive messages of its
  /// interface type.
  pub fn valid(&self) -> bool {
    self.untyped.name.recv_type().has_interface(<U as Case<I>>::VARIANT)
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
