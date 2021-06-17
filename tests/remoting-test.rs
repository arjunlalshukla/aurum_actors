use async_trait::async_trait;
use aurum::core::{Actor, ActorContext, ActorRef, ActorSignal, Host, Node, NodeConfig, Socket};
use aurum_macros::{unify, AurumInterface};
use crossbeam::channel::{unbounded, Sender};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt::Debug;
use std::time::Duration;
use tokio_test::block_on;

#[derive(AurumInterface, Hash, Eq, PartialEq, Debug, Serialize, Deserialize, Clone)]
#[aurum]
enum RemoteLoggerMsg {
  Info(String),
  #[aurum]
  Warning(String),
  #[aurum]
  Error(i32),
}
struct Logger {
  tester: Sender<RemoteLoggerMsg>,
}
#[async_trait]
impl Actor<RemoteTestTypes, RemoteLoggerMsg> for Logger {
  async fn recv(
    &mut self,
    _: &ActorContext<RemoteTestTypes, RemoteLoggerMsg>,
    msg: RemoteLoggerMsg,
  ) {
    self.tester.send(msg).unwrap();
  }

  async fn post_stop(&mut self, _: &ActorContext<RemoteTestTypes, RemoteLoggerMsg>) {
    self.tester.send(RemoteLoggerMsg::Error(-1)).unwrap();
    println!("I am dead.");
  }
}

unify!(RemoteTestTypes = RemoteLoggerMsg ; std::string::String | i32);

fn actor_ref_test(double: bool, port: u16) {
  let socket = Socket::new(Host::DNS("127.0.0.1".to_string()), port, 1001);
  let mut config = NodeConfig::default();
  config.socket = socket.clone();
  let node = Node::<RemoteTestTypes>::new_sync(config).unwrap();
  let _lgr_msg = ActorRef::<RemoteTestTypes, RemoteLoggerMsg>::new::<RemoteLoggerMsg>(
    "logger".to_string(),
    socket.clone(),
  );
  let _err_msg =
    ActorRef::<RemoteTestTypes, i32>::new::<RemoteLoggerMsg>("logger".to_string(), socket.clone());
  let _warn_msg = ActorRef::<RemoteTestTypes, String>::new::<RemoteLoggerMsg>(
    "logger".to_string(),
    socket.clone(),
  );
  let (tx, rx) = unbounded();
  node.spawn(
    double,
    Logger {
      tester: tx,
    },
    "logger".to_string(),
    true,
  );

  let errors = 10;
  let warnings = 15;
  let infos = 20;

  let mut expected = HashSet::new();
  for e in 0..errors {
    block_on(_err_msg.move_to(&node, e));
    expected.insert(RemoteLoggerMsg::Error(e));
  }
  for w in 0..warnings {
    let to_send = format!("warning-{}", w);
    block_on(_warn_msg.send(&node, &to_send));
    expected.insert(RemoteLoggerMsg::Warning(to_send));
  }
  for i in 0..infos {
    let to_send = RemoteLoggerMsg::Info(format!("info-{}", i));
    block_on(_lgr_msg.remote_send(&node, &to_send));
    expected.insert(to_send);
  }

  let timeout = Duration::from_secs(5);
  let mut recvd = HashSet::new();
  loop {
    if recvd.len() == expected.len() && recvd == expected {
      break;
    }
    let msg = rx.recv_timeout(timeout).unwrap();
    println!("Received message: {:?}", msg);
    recvd.insert(msg);
  }
  block_on(_lgr_msg.signal(&node, ActorSignal::Term));
  let msg = rx.recv_timeout(timeout).unwrap();
  assert_eq!(msg, RemoteLoggerMsg::Error(-1));
}

#[test]
fn actor_ref_test_single() {
  actor_ref_test(false, 5001);
}

#[test]
fn actor_ref_test_double() {
  actor_ref_test(true, 5002);
}
