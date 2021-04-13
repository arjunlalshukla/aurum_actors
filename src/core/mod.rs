extern crate aurum_macros;

mod actor;
mod actor_ref;
mod actor_tasks_timeout;
mod actor_tasks_unit;
mod node;
mod packets;
mod registry;
mod remoting;
mod udp_receiver;
mod unify;

#[rustfmt::skip]
pub(crate) use {
  actor::local_actor_msg_convert, 
  actor::ActorMsg,
  actor_tasks_unit::unit_secondary, 
  actor_tasks_unit::unit_single, 
  remoting::Destination,
  packets::DatagramHeader, 
  packets::MessageBuilder, 
  packets::MessagePackets,
  packets::deserialize,
  registry::Registry, 
  registry::SerializedRecvr,
  actor_tasks_timeout::run_single_timeout,
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
