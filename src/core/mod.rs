extern crate aurum_macros;

mod actor;
mod double_threaded;
mod interface;
mod node;
mod packets;
mod registry;
mod remoting;
mod single_threaded;
mod udp_receiver;
mod unify;

pub(crate) use {
  actor::ActorMsg,
  actor::local_actor_msg_convert,
  double_threaded::run_secondary,
  interface::Destination,
  packets::DatagramHeader,
  registry::SerializedRecvr,
  remoting::MAX_PACKET_SIZE,
  remoting::serialize,
  single_threaded::run_single,
  udp_receiver::udp_receiver
};

pub use {
  actor::Actor,
  actor::ActorContext,
  actor::ActorName,
  actor::LocalActorMsg,
  interface::ActorRef,
  interface::HasInterface,
  interface::LocalRef,
  interface::SpecificInterface,
  node::Node,
  registry::Registry,
  registry::RegistryMsg,
  remoting::Address,
  remoting::DeserializeError,
  remoting::Host,
  remoting::Socket,
  remoting::deserialize,
  unify::Case,
  unify::UnifiedBounds,
  unify::forge,
};
