use aurum::actor::{ActorRef, Host, Node};
use aurum::actor::{SpecificInterface, DeserializeError};
use aurum::unify::Case;
use aurum::unified;
use interface_proc::AurumInterface;
use aurum::actor::deserialize;
use serde::{Serialize, Deserialize};
use std::sync::Arc;

#[allow(dead_code)] type Unsigned32 = u32;
#[allow(dead_code)] type MaybeString = Option<String>;

#[derive(AurumInterface, Hash, Eq, PartialEq, Debug, Serialize, Deserialize)]
enum LoggerMsg { 
  Info(String),
  #[aurum]
  Warning(String),
  #[aurum]
  Error(u32)
}
impl<Unified> SpecificInterface<Unified> for LoggerMsg where 
 Unified: Eq + Case<String> + Case<u32> {
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
  <Unified as Case<u32>>::VARIANT => Result::Err(DeserializeError::Other(item)),
  <Unified as Case<String>>::VARIANT => Result::Err(DeserializeError::Other(item)),
  _ => Result::Err(DeserializeError::IncompatibleInterface(item))
}
*/

#[derive(Hash, Eq, PartialEq, Debug, Serialize, Deserialize)]
enum DataStoreMsg { Get, Put(String) }

unified! { MsgTypes = LoggerMsg | DataStoreMsg | MaybeString | String | Unsigned32}
type Actress<T> = ActorRef<MsgTypes, T>;

#[allow(dead_code)]
fn match_types(mt: MsgTypes) {
  match mt {
    <MsgTypes as Case<LoggerMsg>>::VARIANT => (),
    <MsgTypes as Case<DataStoreMsg>>::VARIANT => (),
    <MsgTypes as Case<String>>::VARIANT => (),
    <MsgTypes as Case<Unsigned32>>::VARIANT => (),
    <MsgTypes as Case<MaybeString>>::VARIANT => ()
  }
}

fn main() {
  let node = Node::new(Host::DNS("localhost".to_string()), 1000, 1001);
  let _lgr_msg: Actress<LoggerMsg> = 
    <MsgTypes as Case<LoggerMsg>>::forge("logger".to_string(), node.clone());
  let _ds_msg: Actress<DataStoreMsg> = 
    <MsgTypes as Case<DataStoreMsg>>::forge("data-store".to_string(), node);
  println!("{}", std::any::type_name::<ActorRef<MsgTypes, DataStoreMsg>>());
  let w = "warning".to_string();
  println!("translation warning \"{}\" to {:?}", w.clone(), LoggerMsg::from(w));
  let x = 5u32;
  println!("translation warning \"{}\" to {:?}", x.clone(), LoggerMsg::from(x));
}

#[derive(Hash)]
struct Hello { s: Arc<String> }