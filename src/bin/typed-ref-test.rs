use aurum::actor::{ActorRef, Host, Node, HasInterface};
use aurum::actor::{SpecificInterface, DeserializeError};
use aurum::registry::RegistryMsg;
use aurum::unify::Case;
use aurum::unified;
use interface_proc::AurumInterface;
use aurum::actor::deserialize;
use serde::{Serialize, Deserialize};
use std::fmt::Debug;

#[allow(dead_code)] type Unsigned32 = u32;
#[allow(dead_code)] type MaybeString = Option<String>;
type RegMsg = RegistryMsg<MsgTypes>;

#[derive(AurumInterface, Hash, Eq, PartialEq, Debug, Serialize, Deserialize)]
enum LoggerMsg { 
  Info(String),
  #[aurum]
  Warning(String),
  #[aurum]
  Error(u32)
}
impl HasInterface<LoggerMsg> for LoggerMsg {}
impl HasInterface<String> for LoggerMsg {}
impl HasInterface<u32> for LoggerMsg {}
impl<Unified> SpecificInterface<Unified> for LoggerMsg where 
 Unified: Eq + Case<String> + Case<u32> + Debug {
  fn deserialize_as(item: Unified, bytes: Vec<u8>) ->
   Result<Self, DeserializeError<Unified>> {
    if <Unified as Case<u32>>::VARIANT == item {
      match deserialize::<u32>(bytes) {
        Some(res) => Result::Ok(LoggerMsg::from(res)),
        None => Result::Err(DeserializeError::Other(item))
      }
    } else if <Unified as Case<String>>::VARIANT == item {
      match deserialize::<String>(bytes) {
        Some(res) => Result::Ok(LoggerMsg::from(res)),
        None => Result::Err(DeserializeError::Other(item))
      }      
    } else {
      Result::Err(DeserializeError::IncompatibleInterface(item))
    }
  }
}

/* 
match item {
  <Unified as Case<u32>>::VARIANT => (),
  <Unified as Case<String>>::VARIANT => (),
  _ => ()
}
*/

#[derive(Hash, Eq, PartialEq, Debug, Serialize, Deserialize)]
enum DataStoreMsg { Get, Put(String) }

unified! { MsgTypes = LoggerMsg | DataStoreMsg | MaybeString | String | Unsigned32 | RegMsg}

fn main() {
  let node = Node::new(Host::DNS("localhost".to_string()), 1000, 1001);
  let _lgr_msg = <MsgTypes as Case<LoggerMsg>>::forge
    ::<LoggerMsg>("logger".to_string(), node.clone());
  println!("logger ref: {:#?}", _lgr_msg);
  let _err_msg = <MsgTypes as Case<LoggerMsg>>::forge
    ::<u32>("registry".to_string(), node.clone());
  println!("errors ref: {:#?}", _err_msg);
  let w = "warning".to_string();
  println!("translation warning \"{}\" to {:?}", w.clone(), LoggerMsg::from(w));
  let x = 5u32;
  println!("translation warning \"{}\" to {:?}", x.clone(), LoggerMsg::from(x));
}