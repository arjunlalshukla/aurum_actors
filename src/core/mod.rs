extern crate aurum_macros;

mod actor;
pub(crate) use actor::ActorMsg;
pub use actor::{
  local_actor_msg_convert, Actor, ActorContext, ActorName, LocalActorMsg,
};

mod double_threaded;
pub(crate) use double_threaded::run_secondary;

mod interface;
pub(crate) use interface::Destination;
pub use interface::{ActorRef, HasInterface, LocalRef, SpecificInterface};

mod registry;
pub use registry::{Registry, RegistryMsg, SerializedRecvr};

mod remoting;
pub(crate) use remoting::MAX_PACKET_SIZE;
pub use remoting::{
  deserialize, serialize, Address, DatagramHeader, DeserializeError, Host,
  Socket,
};

mod node;
pub use node::Node;

mod single_threaded;
pub(crate) use single_threaded::run_single;

mod udp_receiver;
pub(crate) use udp_receiver::udp_receiver;

mod unify;
pub use unify::{forge, Case, UnifiedBounds};
