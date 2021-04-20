use crate::core::{
  local_actor_msg_convert, udp_msg, udp_signal, ActorSignal, Case, Destination,
  LocalActorMsg, Socket, UnifiedBounds,
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

#[derive(Clone, Deserialize, Serialize)]
#[serde(bound = "U: Serialize + DeserializeOwned")]
pub struct ActorRef<U: UnifiedBounds + Case<S>, S> {
  pub(in crate::core) socket: Socket,
  pub(in crate::core) dest: Destination<U, S>,
  #[serde(skip, default)]
  pub(in crate::core) local: Option<LocalRef<S>>,
}
impl<U: UnifiedBounds + Case<S>, S: Send + 'static> ActorRef<U, S> {
  pub fn local(&self) -> &Option<LocalRef<S>> {
    &self.local
  }
}
impl<U: UnifiedBounds + Case<S>, S> ActorRef<U, S>
where
  S: Serialize + DeserializeOwned,
{
  pub async fn remote_send(&self, item: &S) {
    udp_msg(&self.socket, &self.dest, item).await;
  }
}
impl<U: UnifiedBounds + Case<S>, S> ActorRef<U, S>
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
impl<U: UnifiedBounds + Case<S>, S: Send> PartialEq for ActorRef<U, S> {
  fn eq(&self, other: &Self) -> bool {
    self.socket == other.socket && self.dest == other.dest
  }
}
impl<U: UnifiedBounds + Case<S>, S: Send> Eq for ActorRef<U, S> {}
impl<U: UnifiedBounds + Case<S>, S: Send> Hash for ActorRef<U, S> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.socket.hash(state);
    self.dest.hash(state);
  }
}
impl<U: UnifiedBounds + Case<S>, S: Send> Debug for ActorRef<U, S> {
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
