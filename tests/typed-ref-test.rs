use aurum::core::{
  forge, serialize, Case, Host, LocalActorMsg, Socket, SpecificInterface,
};
use aurum_macros::{unify, AurumInterface};
use serde::{Deserialize, Serialize};
use std::{any::TypeId, fmt::Debug};

#[derive(
  AurumInterface, Hash, Eq, PartialEq, Debug, Serialize, Deserialize,
)]
#[aurum]
enum LoggerMsg {
  Info(String),
  #[aurum]
  Warning(String),
  #[aurum]
  Error(u32),
}

#[derive(AurumInterface, PartialEq, Eq, Debug)]
#[aurum(local)]
#[allow(dead_code)]
enum DataStoreMsg {
  #[aurum]
  Cmd { cmd: DataStoreCmd },
  #[aurum(local)]
  Subscribe(TypeId),
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
enum DataStoreCmd {
  Get,
  Put(String),
}

unify!(
  TypedRefTypes =
    DataStoreMsg | LoggerMsg | std::string::String | DataStoreCmd | u32
);

#[test]
fn serde_test() {
  let ser_u32 = serialize(LocalActorMsg::Msg(5u32)).unwrap();
  let de_u32 = <LoggerMsg as SpecificInterface<TypedRefTypes>>::deserialize_as(
    <TypedRefTypes as Case<u32>>::VARIANT,
    ser_u32.as_slice(),
  );
  assert_eq!(de_u32.unwrap(), LocalActorMsg::Msg(LoggerMsg::Error(5)));

  let ser_string = serialize(LocalActorMsg::Msg("oh no!".to_string())).unwrap();
  let de_string =
    <LoggerMsg as SpecificInterface<TypedRefTypes>>::deserialize_as(
      <TypedRefTypes as Case<String>>::VARIANT,
      ser_string.as_slice(),
    );
  assert_eq!(
    de_string.unwrap(),
    LocalActorMsg::Msg(LoggerMsg::Warning("oh no!".to_string()))
  );

  let ser_info =
    serialize(LocalActorMsg::Msg(LoggerMsg::Info("hello".to_string())))
      .unwrap();
  let de_info = <LoggerMsg as SpecificInterface<TypedRefTypes>>::deserialize_as(
    <TypedRefTypes as Case<LoggerMsg>>::VARIANT,
    ser_info.as_slice(),
  );
  assert_eq!(
    de_info.unwrap(),
    LocalActorMsg::Msg(LoggerMsg::Info("hello".to_string()))
  );

  let ser_get = serialize(LocalActorMsg::Msg(DataStoreCmd::Get)).unwrap();
  let de_get =
    <DataStoreMsg as SpecificInterface<TypedRefTypes>>::deserialize_as(
      <TypedRefTypes as Case<DataStoreCmd>>::VARIANT,
      ser_get.as_slice(),
    );
  assert_eq!(
    de_get.unwrap(),
    LocalActorMsg::Msg(DataStoreMsg::Cmd {
      cmd: DataStoreCmd::Get
    })
  );

  let ser_put =
    serialize(LocalActorMsg::Msg(DataStoreCmd::Put("put".to_string())))
      .unwrap();
  let de_put =
    <DataStoreMsg as SpecificInterface<TypedRefTypes>>::deserialize_as(
      <TypedRefTypes as Case<DataStoreCmd>>::VARIANT,
      ser_put.as_slice(),
    );
  assert_eq!(
    de_put.unwrap(),
    LocalActorMsg::Msg(DataStoreMsg::Cmd {
      cmd: DataStoreCmd::Put(String::from("put"))
    })
  );
}

#[test]
fn forge_test() {
  let node = Socket::new(Host::DNS("localhost".to_string()), 1000, 1001);
  let _lgr_msg = forge::<TypedRefTypes, LoggerMsg, LoggerMsg>(
    "logger".to_string(),
    node.clone(),
  );
  println!("logger ref: {:#?}", _lgr_msg);

  let _err_msg =
    forge::<TypedRefTypes, LoggerMsg, u32>("logger".to_string(), node.clone());
  println!("logger ref u32 interface: {:#?}", _err_msg);

  let _warn_msg = forge::<TypedRefTypes, LoggerMsg, String>(
    "logger".to_string(),
    node.clone(),
  );
  println!("logger ref string interface: {:#?}", _warn_msg);

  let _ds_msg = forge::<TypedRefTypes, DataStoreMsg, DataStoreCmd>(
    "data_store".to_string(),
    node.clone(),
  );
  println!("data store ref: {:#?}", _ds_msg);
}
