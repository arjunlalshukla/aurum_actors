//! [`Aurum`](crate)'s base functionality: spawning actors, binding sockets, forging actor
//! references and sending messages. 
//! 
//! ### Unifying Types
//! The programmer needs to enumerate to [`Aurum`](crate) all types of messages actors
//! can receive thoughout their whole application. All applications using [`Aurum`](crate) must use
//! exactly one enumeration (henceforth referred to as the
//! [`UnifiedType`](crate::core::UnifiedType)). No more, no less. Due to the way the 
//! [`UnifiedType`](crate::core::UnifiedType)
//! is handled, it is still possible to build and use many independent libraries on top of
//! [`Aurum`](crate). The unified type is constructed with the [`unify`](crate::unify) macro. See
//! the [`unify`](crate::unify) macro for an in-depth discussion on how it works and why we need it.
//! 
//! ### Actor References
//! Actor References come in 2 flavors: [`ActorRef`](crate::core::ActorRef) and
//! [`LocalRef`](crate::core::LocalRef). [`ActorRef`](crate::core::ActorRef) is
//! location-transparent, it accepts both local and remote messages.
//! [`LocalRef`](crate::core::LocalRef) is for local messages only. An 
//! [`ActorRef`](crate::core::ActorRef) requires 2 generic parameters: the
//! [`UnifiedType`](crate::core::UnifiedType) and the interface (not the root message type).
//! 
//! ### Creating a Message Type
//! [`AurumInterface`](crate::AurumInterface) is a derive macro applied to the message types for
//! actors. It generates all the necessary implementations for the message type to interact with
//! the [`UnifiedType`](crate::core::UnifiedType) and produce interfaces.
//! [`AurumInterface`](crate::AurumInterface) has a single annotation: `aurum`, which has one
//! optional argument, telling [`AurumInterface`](crate::AurumInterface) whether the interface is
//! to be exclusively local or not. See [`AurumInterface`](crate::AurumInterface) for details.
//! 
//! ```ignore
//! #[derive(AurumInterface)]
//! #[aurum(local)]
//! enum MyMsgType {
//!   #[aurum]
//!   MyStringInterface(String),
//!   #[aurum(local)]
//!   MyNonserializableInterface(&'static str),
//!   MyOtherMsg(usize),
//! }
//! ```
//! 
//! In this example, because one of the message options is not serializable, the message type as a
//! whole is not serializable. However, you can use an
//! [`ActorRef<MyUnifiedType, String>`](crate::core::ActorRef) to send a string from a remote
//! machine to whatever actor uses this message type. You can also create a
//! [`LocalRef<&’static str>`](crate::core::LocalRef), but not a usable
//! [`ActorRef`](crate::core::ActorRef).
//! 
//! ### A [`unify`](crate::unify) Example
//! The [`unify`](crate::unify) macro is responsible for constructing the
//! [`UnifiedType`](crate::core::UnifiedType) and implementing traits for it. Arguments must include
//! all message types (whether they are to be remotely accessed or not), and types used for remote
//! interfaces. [`unify`](crate::unify) creates a type, so it should only be called once in a single
//! application. 
//! 
//! ```ignore
//! unify! { MyUnifiedType =
//!   MyMsgType |
//!   MyOtherMsgType |
//!   MsgTypeForSomeThirdPartyLibrary
//!   ;
//!   String |
//!   InterfaceForSomeThirdPartyLibrary
//! }
//! ```
//! 
//! ### Spawning Actors
//! To spawn actors, you need to create a [`Node`](crate::core::Node). You also need an initial
//! instance of whatever type implements the actor trait, the string part of the actor’s name,
//! whether the actor should be double-threaded and whether a reference to it should be sent to the
//! registry.
//! 
//! ```ignore
//! struct MyActor {
//!   first: String,
//!   second: usize
//! }
//! //Don’t forget this annotation
//! #[async_trait]
//! impl Actor<MyUnifiedType, MyMsgType> for MyActor { ... }
//! 
//! let mut config = NodeConfig::default();
//! config.socket = Socket::new(...);
//! let node = Node::new(config);
//! let actor = MyActor {
//!   first: "hi  there".to_string(),
//!   second: 4214
//! };
//! node.spawn(
//!   false, // Double -threaded?1
//!   actor,
//!   "my-very-cool-actor".to_string(),
//!   true, // Register
//! );
//! ```
//! 
//! ### Sending Messages
//! An [`ActorId`] is used to uniquely identify an actor in the registry of a [`Node`]. A
//! [`Destination`] contains an [`ActorId`] and information on which interface it is sending
//! messages to. A [`Destination`] can be forged, just like an [`ActorRef`]. The combination of a
//! [`Socket`] and a [`Destination`] consitutes a complete actor address. You can construct and use
//! these two components indepentently.
//! 
//! ```ignore
//! let socket = Socket::new(...);
//! 
//! // Forging a Destination
//! let dest = Destination::<MyUnifiedType, String>::new::<MyMsgType>(
//!   "my-very-cool-actor".to_string()
//! );
//! let msg = "My name is Baloo".to_string();
//! let ser = UdpSerial::new(&dest, &msg)
//! node.udp(&socket, &ser);
//! 
//! // Forging an ActorRef
//! let forged = ActorRef::<MyUnifiedType, String>::new::<MyMsgType>(
//!   "my-very-cool-actor".to_string(),
//!   socket.clone()
//! );
//! forged.remote_send(&node, &msg) ;
//! ```
//! 
//! You can send a message unreliably using [`Node.udp_select`](crate::core::Node::udp_select).
//! How messages are dropped or delayed depends on a [`FailureMode`](crate::testkit::FailureMode)
//! and a [`FailureConfigMap`](crate::testkit::FailureConfigMap). 
//! 
//! ```ignore
//! // Doesn’t have to be a const
//! const MODE: FailureMode = FailureMode :: Message ;
//! let mut fail_map = FailureConfigMap::default();
//! // Make some changes to fail_map...
//! let msg: String = "My name is Baloo".to_string();
//! let ser = UdpSerial::new(&dest, &msg);
//! node.udp_select(&socket , &ser, MODE, &fail_map) ;
//! ```
//! 
//! ### Logging
//! Define a [`LogLevel`] for you environment, and call one of these log macros. They all have the same
//! arguments.
//! 
//! - [`trace`](crate::trace)
//! - [`debug`](crate::debug)
//! - [`info`](crate::info)
//! - [`warn`](crate::warn)
//! - [`error`](crate::error)
//! - [`fatal`](crate::fatal)
//! 
//! The logger is accessible from the node, but there is nothing stopping you from spawning your own
//! logger, it’s just an actor. The log messages can be anything that implements
//! [`Display`](std::fmt::Display). The argument is turned into a trait object in the macro body.
//! 
//! ```ignore
//! // Doesn’t have to be a const
//! const LEVEL: LogLevel = LogLevel::Debug ;
//! // Not logged, the level is Debug, which is above Trace
//! trace!(LEVEL, &node, "kelp") ;
//! // This is logged, Warn is above Debug
//! warn!(LEVEL, &node, "sharks") ;
//! ```
//! 
//! 

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
mod unification;

pub const LOG_LEVEL: LogLevel = LogLevel::Error;

#[rustfmt::skip]
pub(in crate::core) use {
  actor::local_actor_msg_convert, 
  actor::ActorMsg,
  actor_tasks_unit::unit_secondary, 
  actor_tasks_unit::unit_single, 
  packets::DatagramHeader, 
  packets::MessageBuilder, 
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
  actor::ActorContext, 
  actor::ActorId, 
  actor::ActorSignal,
  actor::TimeoutActor,
  actor_ref::ActorRef, 
  actor_ref::LocalRef,
  remoting::Destination,
  remoting::Host, 
  remoting::Socket,
  node::Node,
  node::NodeConfig,
  udp_serial::UdpSerial,
  unification::Case, 
  unification::RootMessage, 
  unification::UnifiedType,
};
