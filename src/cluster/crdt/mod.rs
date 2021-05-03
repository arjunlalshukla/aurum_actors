use serde::{de::DeserializeOwned, Serialize};
use std::hash::Hash;

pub trait CRDT
where
  Self: Clone
    + DeserializeOwned
    + Eq
    + PartialEq
    + Hash
    + Send
    + Serialize
    + Sized
    + Sync
    + 'static,
{
  type Delta: DeltaMutator<Self> + Send;
  fn delta(&self, changes: &Self::Delta) -> Self;
  fn empty(&self) -> bool;
  fn join(self, other: Self) -> Self;
  fn minimum() -> Self;
}

pub trait DeltaMutator<T> {
  fn apply(&self, target: &T) -> T;
}

mod causal;
mod test_crdt;

pub use {causal::CausalCmd, causal::CausalDisperse};
