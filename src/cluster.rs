use crate::unify::Case;
use serde::{Serialize, Deserialize};
use std::marker::PhantomData;

#[derive(Serialize, Deserialize)]
enum IntraClusterMsg {
  Heartbeat,
  State,
  HeartbeatRequest
}

struct Cluster<Unified> {
  p: PhantomData<Unified>
}
impl<Unified> Cluster<Unified> where Unified: Case<IntraClusterMsg> {
  fn foo() {
    <Unified as Case<IntraClusterMsg>>::name("hi there".to_string());
  }
}
