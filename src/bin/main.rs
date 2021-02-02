use aurum::actor_ref::{ActorRef, Host, Node};
use aurum::unify::ForgeRef;
use aurum::unified;

#[derive(Hash, Eq, PartialEq, Debug)]
enum ClusterUpdate {
  Added,
  Deleted
}

#[derive(Hash, Eq, PartialEq, Debug)]
#[allow(dead_code)]
enum LoggerMsg { 
  Info(String), 
  Warning(String), 
  Error(String),
  Cluster(ClusterUpdate)
}

#[allow(dead_code)]
#[derive(Hash, Eq, PartialEq, Debug)]
enum DataStoreMsg { Get, Put(String) }

unified! { MsgTypes = LoggerMsg | DataStoreMsg }
type Actress<T> = ActorRef<MsgTypes, T>;

#[allow(dead_code)]
fn foo(mt: MsgTypes) {
  match mt {
    MsgTypes::LoggerMsg(s) => println!("{}", s),
    MsgTypes::DataStoreMsg(s) => println!("{}", s)
  };
}

fn main() {
  let node = Node::new(Host::DNS("localhost".to_string()), 1000, 1001);
  let lgr_msg: Actress<LoggerMsg> = 
    <MsgTypes as ForgeRef<LoggerMsg>>::forge("logger".to_string(), node.clone());
  let ds_msg: Actress<DataStoreMsg> = 
    <MsgTypes as ForgeRef<DataStoreMsg>>::forge("data-store".to_string(), node);
  println!("logger-recvr: {:?}", lgr_msg);
  println!("data-store-recvr: {:?}", ds_msg);
}