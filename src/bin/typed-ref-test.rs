use aurum::actor_ref::{ActorRef, Host, Node};
use aurum::unify::Case;
use aurum::unified;
use serde::{Serialize, Deserialize};
use std::sync::Arc;

type MaybeString = Option<String>;

#[derive(Hash, Eq, PartialEq, Debug, Serialize, Deserialize)]
#[allow(dead_code)]
enum LoggerMsg { 
  Info(String), 
  Warning(String), 
  Error(String)
}

#[allow(dead_code)]
#[derive(Hash, Eq, PartialEq, Debug, Serialize, Deserialize)]
enum DataStoreMsg { Get, Put(String) }

unified! { MsgTypes = LoggerMsg | DataStoreMsg | MaybeString }
type Actress<T> = ActorRef<MsgTypes, T>;

#[allow(dead_code)]
fn foo(mt: MsgTypes) {
  match mt {
    MsgTypes::LoggerMsg(s) => println!("{}", s),
    MsgTypes::DataStoreMsg(s) => println!("{}", s),
    MsgTypes::MaybeString(_) => ()
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
  println!("{}", std::any::type_name::<ActorRef<MsgTypes, DataStoreMsg>>())
}

#[derive(Hash)]
struct Hello { s: Arc<String> }