use aurum::actor_ref::{ActorRef, Host, Node};
use aurum::unify::Case;
use aurum::actor::Translatable;
use aurum::unified;
use interface_proc::AurumInterface;
use serde::{Serialize, Deserialize};
use std::sync::Arc;

type MaybeString = Option<String>;

#[derive(AurumInterface, Hash, Eq, PartialEq, Debug, Serialize, Deserialize)]
enum LoggerMsg { 
  Info(String),
  #[aurum]
  Warning(String),
  #[aurum]
  Error(u32)
}

#[allow(unused)]
#[derive(Hash, Eq, PartialEq, Debug, Serialize, Deserialize)]
enum DataStoreMsg { Get, Put(String) }

unified! { MsgTypes = LoggerMsg | DataStoreMsg | MaybeString }
type Actress<T> = ActorRef<MsgTypes, T>;

#[allow(unused)]
fn foo(mt: MsgTypes) {
  match mt {
    MsgTypes::LoggerMsg => println!("Matched logger"),
    MsgTypes::DataStoreMsg => println!("Matched data store"),
    MsgTypes::MaybeString => println!("Matched maybe string")
  };
}

fn main() {
  let node = Node::new(Host::DNS("localhost".to_string()), 1000, 1001);
  let lgr_msg: Actress<LoggerMsg> = 
    <MsgTypes as Case<LoggerMsg>>::forge("logger".to_string(), node.clone());
  let ds_msg: Actress<DataStoreMsg> = 
    <MsgTypes as Case<DataStoreMsg>>::forge("data-store".to_string(), node);
  println!("logger-recvr: {:?}", lgr_msg);
  println!("data-store-recvr: {:?}", ds_msg);
  println!("{}", std::any::type_name::<ActorRef<MsgTypes, DataStoreMsg>>());
  let w = "warning".to_string();
  println!("translation warning \"{}\" to {:?}", w.clone(), 
    <LoggerMsg as Translatable<String>>::translate(w));
  let x = 5u32;
  println!("translation warning \"{}\" to {:?}", x.clone(), 
    <LoggerMsg as Translatable<u32>>::translate(x));
}

#[derive(Hash)]
struct Hello { s: Arc<String> }