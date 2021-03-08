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

#[rustfmt::skip]
pub(crate) use {
  actor::local_actor_msg_convert, 
  actor::ActorMsg,
  double_threaded::run_secondary, 
  interface::Destination,
  packets::DatagramHeader, 
  packets::MessageBuilder, 
  packets::MessagePackets,
  registry::Registry, 
  registry::SerializedRecvr, 
  remoting::serialize, 
  single_threaded::run_single,
  udp_receiver::udp_receiver,
};

// Needed for macros
#[rustfmt::skip]
pub use {
  actor::LocalActorMsg, 
  interface::HasInterface,
  interface::SpecificInterface, 
  remoting::deserialize,
  remoting::DeserializeError, 
  unify::Case, 
  unify::UnifiedBounds,
};

// Actual public interface
#[rustfmt::skip]
pub use {
  actor::Actor, 
  actor::ActorContext, 
  actor::ActorName, 
  actor::ActorSignal,
  interface::ActorRef, 
  interface::LocalRef, 
  node::Node,
  registry::RegistryMsg,
  remoting::Host, 
  remoting::Socket, 
  unify::forge,
};
