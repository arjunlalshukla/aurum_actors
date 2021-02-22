extern crate aurum_macros;

mod actor;
pub(crate) use actor::ActorMsg;
pub use actor::{
  local_actor_msg_convert, Actor, ActorContext, ActorName, LocalActorMsg,
};

mod double_threaded;
pub(crate) use double_threaded::run_secondary;

mod interface;
pub use interface::{ActorRef, HasInterface, LocalRef, SpecificInterface};

mod registry;
pub use registry::{Registry, RegistryMsg, SerializedRecvr};

mod remoting;
pub use remoting::{
  deserialize, serialize, Address, DeserializeError, Host, Socket,
};

mod node;
pub use node::Node;

mod single_threaded;
pub(crate) use single_threaded::run_single;

mod unify;
pub use unify::{forge, Case, UnifiedBounds};
