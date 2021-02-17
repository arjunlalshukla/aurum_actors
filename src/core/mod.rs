extern crate aurum_macros;

mod actor;
pub use actor::{Actor, ActorContext, ActorMsg, ActorName};

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

mod unify;
pub use unify::{forge, Case, UnifiedBounds};
