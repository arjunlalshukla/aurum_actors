use crate::core::{
  local_actor_msg_convert, udp_msg, udp_signal, ActorSignal, Case, Destination,
  LocalActorMsg, Socket, SpecificInterface, UnifiedType,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::cmp::PartialEq;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;

pub struct LocalRef<T> {
  pub(crate) func: Arc<dyn Fn(LocalActorMsg<T>) -> bool + Send + Sync>,
}
impl<T> Clone for LocalRef<T> {
  fn clone(&self) -> Self {
    LocalRef {
      func: self.func.clone(),
    }
  }
}
impl<T: Send + 'static> LocalRef<T> {
  pub fn send(&self, item: T) -> bool {
    (&self.func)(LocalActorMsg::Msg(item))
  }

  pub fn signal(&self, sig: ActorSignal) -> bool {
    (&self.func)(LocalActorMsg::Signal(sig))
  }

  pub fn transform<I: Send + 'static>(&self) -> LocalRef<I>
  where
    T: From<I>,
  {
    let func = self.func.clone();
    LocalRef {
      func: Arc::new(move |x: LocalActorMsg<I>| {
        func(local_actor_msg_convert(x))
      }),
    }
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

#[derive(Deserialize, Serialize)]
#[serde(bound = "U: Serialize + DeserializeOwned")]
pub struct ActorRef<U: UnifiedType + Case<I>, I> {
  pub socket: Socket,
  pub dest: Destination<U, I>,
  #[serde(skip, default)]
  pub(in crate::core) local: Option<LocalRef<I>>,
}
impl<U: UnifiedType + Case<I>, I> ActorRef<U, I> {
  pub fn valid(&self) -> bool {
    self.dest.valid()
  }

  pub fn new<S>(name: String, socket: Socket) -> Self
  where
    U: Case<S>,
    S: From<I> + SpecificInterface<U>,
  {
    Self {
      socket: socket,
      dest: Destination::new::<S>(name),
      local: None,
    }
  }
}
impl<U: UnifiedType + Case<I>, I: Send + 'static> ActorRef<U, I> {
  pub fn local(&self) -> &Option<LocalRef<I>> {
    &self.local
  }
}
impl<U: UnifiedType + Case<I>, I> ActorRef<U, I>
where
  I: Serialize + DeserializeOwned,
{
  pub async fn remote_send(&self, item: &I) {
    udp_msg(&self.socket, &self.dest, item).await;
  }
}
impl<U: UnifiedType + Case<S>, S> ActorRef<U, S>
where
  S: Send + Serialize + DeserializeOwned + 'static,
{
  pub async fn send(&self, item: &S) -> Option<bool>
  where
    S: Clone,
  {
    if let Some(r) = &self.local {
      Some(r.send(item.clone()))
    } else {
      udp_msg(&self.socket, &self.dest, item).await;
      None
    }
  }

  pub async fn move_to(&self, item: S) -> Option<bool> {
    if let Some(r) = &self.local {
      Some(r.send(item))
    } else {
      udp_msg(&self.socket, &self.dest, &item).await;
      None
    }
  }

  pub async fn signal(&self, sig: ActorSignal) -> Option<bool> {
    if let Some(r) = &self.local {
      Some(r.signal(sig))
    } else {
      udp_signal(&self.socket, &self.dest, &sig).await;
      None
    }
  }
}
impl<U: UnifiedType + Case<S>, S: Send> Clone for ActorRef<U, S> {
  fn clone(&self) -> Self {
    Self {
      socket: self.socket.clone(),
      dest: self.dest.clone(),
      local: self.local.clone(),
    }
  }
}
impl<U: UnifiedType + Case<S>, S: Send> PartialEq for ActorRef<U, S> {
  fn eq(&self, other: &Self) -> bool {
    self.socket == other.socket && self.dest == other.dest
  }
}
impl<U: UnifiedType + Case<S>, S: Send> Eq for ActorRef<U, S> {}
impl<U: UnifiedType + Case<S>, S: Send> Hash for ActorRef<U, S> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.socket.hash(state);
    self.dest.hash(state);
  }
}
impl<U: UnifiedType + Case<S>, S: Send> Debug for ActorRef<U, S> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("ActorRef")
      .field("Unified", &std::any::type_name::<U>())
      .field("Specific", &std::any::type_name::<S>())
      .field("socket", &self.socket)
      .field("dest", &self.dest)
      .field("has_local", &self.local.is_some())
      .finish()
  }
}

#[cfg(test)]
use crate::core::{deserialize, serialize, Host};

#[cfg(test)]
mod ref_safety {

  use crate as aurum;
  use crate::{unify, AurumInterface};
  use serde::{Deserialize, Serialize};

  #[derive(AurumInterface, Serialize, Deserialize)]
  pub enum Foo {
    #[aurum]
    One(String),
  }

  #[derive(AurumInterface, Serialize, Deserialize)]
  pub enum Bar {
    #[aurum]
    One(String),
    #[aurum]
    Two(i32),
  }

  unify!(pub Quz = Foo | Bar ; String | i32);
}
#[cfg(test)]
use ref_safety::*;

#[test]
#[cfg(test)]
#[allow(dead_code)]
fn actor_ref_valid_test() {
  let socket = Socket::new(Host::DNS("localhost".to_string()), 0, 0);
  let foo = ActorRef::<Quz, Foo>::new::<Foo>("foo".to_string(), socket.clone());
  let ser = serialize(&foo).unwrap();
  let invalid = deserialize::<ActorRef<Quz, Bar>>(&ser[..]).unwrap();
  assert!(!invalid.valid());
  let invalid = deserialize::<ActorRef<Quz, i32>>(&ser[..]).unwrap();
  assert!(!invalid.valid());
  let valid = deserialize::<ActorRef<Quz, Foo>>(&ser[..]).unwrap();
  assert!(valid.valid());
  let valid = deserialize::<ActorRef<Quz, String>>(&ser[..]).unwrap();
  assert!(valid.valid());
}
