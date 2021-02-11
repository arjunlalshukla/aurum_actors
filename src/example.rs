enum ClusterUpdate {
  Add,
  Delete
}

// ActorRef<ClusterUpdate>

enum OurActorMsg {
  Report(String),
  // #[aurum interface]
  ClusterUpdateMsg(ClusterUpdate)
}

impl From<ClusterUpdate> for OurActorMsg {
  fn from(c: ClusterUpdate) -> OurActorMsg {
    OurActorMsg::ClusterUpdateMsg(c)
  }
}