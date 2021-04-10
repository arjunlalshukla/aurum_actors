extern crate aurum_macros;

mod actor;
mod actor_ref;
mod double_threaded;
mod node;
mod packets;
mod registry;
mod remoting;
mod single_threaded;
mod single_timeout;
mod udp_receiver;
mod unify;

#[rustfmt::skip]
pub(crate) use {
  actor::local_actor_msg_convert, 
  actor::ActorMsg,
  double_threaded::run_secondary, 
  remoting::Destination,
  packets::DatagramHeader, 
  packets::MessageBuilder, 
  packets::MessagePackets,
  packets::deserialize,
  registry::Registry, 
  registry::SerializedRecvr,
  single_threaded::run_single,
  single_timeout::run_single_timeout,
  udp_receiver::udp_receiver,
};

// Needed for macros
#[rustfmt::skip]
pub use {
  actor::LocalActorMsg,
  registry::RegistryMsg,
  packets::deserialize_msg,
  packets::DeserializeError, 
  packets::Interpretations,
  unify::Case, 
  unify::SpecificInterface, 
  unify::UnifiedBounds,
};

// Actual public interface
#[rustfmt::skip]
pub use {
  actor::Actor, 
  actor::TimeoutActor,
  actor::ActorContext, 
  actor::ActorName, 
  actor::ActorSignal,
  actor_ref::ActorRef, 
  actor_ref::LocalRef, 
  remoting::Host, 
  remoting::Socket, 
  remoting::udp_msg,
  remoting::udp_signal,
  remoting::udp_msg_unreliable,
  remoting::udp_signal_unreliable,
  node::Node,
  unify::forge,
};
