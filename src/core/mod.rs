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
  actor::local_actor_msg_convert, actor::ActorMsg,
  double_threaded::run_secondary, interface::Destination,
  packets::DatagramHeader, packets::MessagePackets, registry::SerializedRecvr,
  remoting::serialize, remoting::MAX_PACKET_SIZE, single_threaded::run_single,
  udp_receiver::udp_receiver,
};

pub use {
  actor::Actor, actor::ActorContext, actor::ActorName, actor::LocalActorMsg,
  interface::ActorRef, interface::HasInterface, interface::LocalRef,
  interface::SpecificInterface, node::Node, registry::Registry,
  registry::RegistryMsg, remoting::deserialize, remoting::DeserializeError,
  remoting::Host, remoting::Socket, unify::forge, unify::Case,
  unify::UnifiedBounds,
};
