use aurum::{actor::{Host, Node, HasInterface}, registry::Registry};
use aurum::actor::{SpecificInterface, DeserializeError};
use aurum::registry::RegistryMsg;
use aurum::unify::Case;
use aurum::unified;
use interface_proc::AurumInterface;
use aurum::actor::deserialize;
use serde::{Serialize, Deserialize};
use std::fmt::Debug;
use std::collections::HashMap;

#[allow(dead_code)] type Unsigned32 = u32;
#[allow(dead_code)] type MaybeString = Option<String>;
#[allow(dead_code)] type RegMsg = RegistryMsg<MsgTypes>;

#[derive(AurumInterface, Hash, Eq, PartialEq, Debug, Serialize, Deserialize)]
enum LoggerMsg { 
  Info(String),
  #[aurum]
  Warning(String),
  #[aurum]
  Error(u32)
}
impl HasInterface<LoggerMsg> for LoggerMsg  {}
impl HasInterface<String> for LoggerMsg {}
impl HasInterface<u32> for LoggerMsg {}
impl<Unified> SpecificInterface<Unified> for LoggerMsg where 
 Unified: Eq + Case<LoggerMsg> + Case<String> + Case<u32> + Debug {
  fn deserialize_as(item: Unified, bytes: Vec<u8>) ->
   Result<Self, DeserializeError<Unified>> {
    if <Unified as Case<LoggerMsg>>::VARIANT == item {
      deserialize::<Unified, LoggerMsg, LoggerMsg>(item, bytes)
    } else if <Unified as Case<u32>>::VARIANT == item {
      deserialize::<Unified, LoggerMsg, u32>(item, bytes)
    } else if <Unified as Case<String>>::VARIANT == item {
      deserialize::<Unified, LoggerMsg, String>(item, bytes)
    } else {
      Result::Err(DeserializeError::IncompatibleInterface(item))
    }
  }
}

#[derive(Hash, Eq, PartialEq, Debug, Serialize, Deserialize)]
enum DataStoreMsg { Get, Put(String) }

unified! { MsgTypes = LoggerMsg | DataStoreMsg | MaybeString | String | Unsigned32 | RegMsg}

fn main() {
  let r = Registry::<MsgTypes> {register: HashMap::new()};

  let node = Node::new(Host::DNS("localhost".to_string()), 1000, 1001);
  let _lgr_msg = <MsgTypes as Case<LoggerMsg>>::forge
    ::<LoggerMsg>("logger".to_string(), node.clone());
  println!("logger ref: {:#?}", _lgr_msg);
  let _err_msg = <MsgTypes as Case<LoggerMsg>>::forge
    ::<u32>("logger".to_string(), node.clone());
  println!("errors ref: {:#?}", _err_msg);
  let w = "warning".to_string();
  println!("translation warning \"{}\" to {:?}", w.clone(), LoggerMsg::from(w));
  let x = 5u32;
  println!("translation warning \"{}\" to {:?}", x.clone(), LoggerMsg::from(x));
}