use crate::actor::Translatable;

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

impl Translatable<ClusterUpdate> for OurActorMsg {
  fn translate(c: ClusterUpdate) -> OurActorMsg {
    OurActorMsg::ClusterUpdateMsg(c)
  }
}