use aurum::core::{Case, Host, Node, RegistryMsg, SpecificInterface, serialize};
use aurum::unified;
use interface_proc::AurumInterface;
use serde::{Serialize, Deserialize};
use std::fmt::Debug;

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

unified! { MsgTypes = DataStoreMsg | LoggerMsg | MaybeString | RegMsg | String |
  Unsigned32 }

#[test]
fn serde_test() {
  let ser_u32 = serialize(5u32).unwrap();
  let de_u32 = <LoggerMsg as SpecificInterface<MsgTypes>>
    ::deserialize_as(<MsgTypes as Case<u32>>::VARIANT, ser_u32);
  assert_eq!(de_u32.unwrap(), LoggerMsg::Error(5));

  let ser_string = serialize("oh no!".to_string()).unwrap();
  let de_string = <LoggerMsg as SpecificInterface<MsgTypes>>
    ::deserialize_as(<MsgTypes as Case<String>>::VARIANT, ser_string);
  assert_eq!(de_string.unwrap(), LoggerMsg::Warning("oh no!".to_string()));

  let ser_info = serialize(LoggerMsg::Info("hello".to_string())).unwrap();
  let de_info = <LoggerMsg as SpecificInterface<MsgTypes>>
    ::deserialize_as(<MsgTypes as Case<LoggerMsg>>::VARIANT, ser_info);
  assert_eq!(de_info.unwrap(), LoggerMsg::Info("hello".to_string()));
  
  let ser_get = serialize(DataStoreMsg::Get).unwrap();
  let de_get = <DataStoreMsg as SpecificInterface<MsgTypes>>
    ::deserialize_as(<MsgTypes as Case<DataStoreMsg>>::VARIANT, ser_get);
  assert_eq!(de_get.unwrap(), DataStoreMsg::Get);

  let ser_put = serialize(DataStoreMsg::Put("put".to_string())).unwrap();
  let de_put = <DataStoreMsg as SpecificInterface<MsgTypes>>
    ::deserialize_as(<MsgTypes as Case<DataStoreMsg>>::VARIANT, ser_put);
  assert_eq!(de_put.unwrap(), DataStoreMsg::Put("put".to_string()));
}

#[test]
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
