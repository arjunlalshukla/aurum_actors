extern crate interface_proc;

mod actor;
pub use actor::{
  Actor, 
  ActorContext,
  HiddenInterface
};

mod interface;
pub use interface::{
  ActorRef,
  HasInterface,
  LocalRef,
  SpecificInterface
};

mod registry;
pub use registry::{
  Registry,
  RegistryMsg,
  SerializedRecvr
};

mod remoting;
pub use remoting::{
  Address,
  DeserializeError,
  Host,
  Socket,
  deserialize,
  serialize
};

mod spawn;
pub use spawn::spawn;

mod node;
pub use node::{
  Node
};

mod unify;
pub use unify::{Case, forge};