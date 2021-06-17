extern crate aurum_macros;
use crate::testkit::LogLevel;

mod actor;
mod actor_ref;
mod actor_tasks_timeout;
mod actor_tasks_unit;
mod node;
mod packets;
mod registry;
mod remoting;
mod udp_receiver;
mod udp_serial;
mod unify;

pub const LOG_LEVEL: LogLevel = LogLevel::Error;

#[rustfmt::skip]
pub(in crate::core) use {
  actor::local_actor_msg_convert, 
  actor::ActorMsg,
  actor_tasks_unit::unit_secondary, 
  actor_tasks_unit::unit_single, 
  packets::DatagramHeader, 
  packets::MessageBuilder, 
  packets::MessagePackets,
  packets::deserialize,
  packets::serialize,
  registry::Registry, 
  registry::SerializedRecvr,
  remoting::DestinationUntyped,
  actor_tasks_timeout::run_single_timeout,
  udp_receiver::udp_receiver,
};

// Needed for macros
#[rustfmt::skip]
#[doc(hidden)]
pub use {
  actor::LocalActorMsg,
  registry::RegistryMsg,
  packets::deserialize_msg,
  packets::DeserializeError, 
  packets::Interpretations,
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
  remoting::Destination,
  remoting::Host, 
  remoting::Socket,
  node::Node,
  node::NodeConfig,
  unify::Case, 
  unify::SpecificInterface, 
  unify::UnifiedType,
};
