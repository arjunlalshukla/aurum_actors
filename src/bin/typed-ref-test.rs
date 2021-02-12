use aurum::actor::{DeserializeError, Host, Node, SpecificInterface};
use aurum::registry::RegistryMsg;
use aurum::unify::Case;
use aurum::unified;
use interface_proc::AurumInterface;
use aurum::actor::deserialize;
use serde::{Serialize, Deserialize};
use std::fmt::Debug;
//use std::collections::HashMap;

#[allow(dead_code)] type Unsigned32 = u32;
#[allow(dead_code)] type MaybeString = Option<String>;
#[allow(dead_code)] type RegMsg = RegistryMsg<MsgTypes>;

#[derive(AurumInterface, Hash, Eq, PartialEq, Debug, Serialize, Deserialize)]
#[aurum]
enum LoggerMsg { 
  Info(String),
  #[aurum]
  Warning(String),
  #[aurum]
  Error(u32)
}

#[derive(AurumInterface, Hash, Eq, PartialEq, Debug, Serialize, Deserialize)]
#[aurum]
enum DataStoreMsg { 
  Get, 
  Put(String) 
}

unified! { MsgTypes = LoggerMsg | DataStoreMsg | MaybeString | String | Unsigned32 | RegMsg}

fn main() {
  forge_test();
  serde_test();
}

fn serde_test() {
  let ser_u32 = serde_json::to_string(&5u32).unwrap();
  println!("u32 serialized: {}", ser_u32);

  let ser_string = serde_json::to_string(&String::from("oh no!")).unwrap();
  println!("string serialized: {}", ser_string);

  let ser_lgr = serde_json::to_string(&LoggerMsg::Info(String::from("hello"))).unwrap();
  println!("LoggerMsg::Info serialized: {}", ser_lgr);

  let ser_get = serde_json::to_string(&DataStoreMsg::Get).unwrap();
  println!("DataStoreMsg::Get serialized: {}", ser_get);

  let ser_put = serde_json::to_string(&DataStoreMsg::Put(String::from("put"))).unwrap();
  println!("DataStoreMsg::Put serialized: {}", ser_put);



  let de_u32 = <LoggerMsg as SpecificInterface<MsgTypes>>
    ::deserialize_as(<MsgTypes as Case<u32>>::VARIANT, ser_u32.into_bytes()).unwrap();
  println!("u32 deserialized: {:?}", de_u32);

  let de_string = <LoggerMsg as SpecificInterface<MsgTypes>>
    ::deserialize_as(<MsgTypes as Case<String>>::VARIANT, ser_string.into_bytes()).unwrap();
  println!("string deserialized: {:?}", de_string);

  let de_lgr = <LoggerMsg as SpecificInterface<MsgTypes>>
    ::deserialize_as(<MsgTypes as Case<LoggerMsg>>::VARIANT, ser_lgr.into_bytes()).unwrap();
  println!("LoggerMsg::Info deserialized: {:?}", de_lgr);

  let de_get = <DataStoreMsg as SpecificInterface<MsgTypes>>
    ::deserialize_as(<MsgTypes as Case<DataStoreMsg>>::VARIANT, ser_get.into_bytes()).unwrap();
  println!("DataStoreMsg::Get deserialized: {:?}", de_get);

  let de_put = <DataStoreMsg as SpecificInterface<MsgTypes>>
    ::deserialize_as(<MsgTypes as Case<DataStoreMsg>>::VARIANT, ser_put.into_bytes()).unwrap();
  println!("DataStoreMsg::Put deserialized: {:?}", de_put);
}

fn forge_test() {
  let node = Node::new(Host::DNS("localhost".to_string()), 1000, 1001);
  let _lgr_msg = <MsgTypes as Case<LoggerMsg>>::forge
    ::<LoggerMsg>("logger".to_string(), node.clone());
  println!("logger ref: {:#?}", _lgr_msg);

  let _err_msg = <MsgTypes as Case<LoggerMsg>>::forge
    ::<u32>("logger".to_string(), node.clone());
  println!("logger ref u32 interface: {:#?}", _err_msg);

  let _warn_msg = <MsgTypes as Case<LoggerMsg>>::forge
    ::<String>("logger".to_string(), node.clone());
  println!("logger ref string interface: {:#?}", _warn_msg);

  let _ds_msg = <MsgTypes as Case<DataStoreMsg>>::forge
    ::<DataStoreMsg>("data_store".to_string(), node.clone());
  println!("data store ref: {:#?}", _ds_msg);
}
