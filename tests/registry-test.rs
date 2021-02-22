use async_trait::async_trait;
use aurum::core::{
  serialize, Actor, ActorContext, ActorName, Case, Host, LocalActorMsg, Node,
  RegistryMsg, Socket,
};
use aurum_macros::{unify, AurumInterface};
use crossbeam::channel::{bounded, unbounded, Sender};
use serde::{Deserialize, Serialize};

#[derive(AurumInterface, PartialEq, Eq, Serialize, Deserialize, Debug)]
enum Echo {
  #[aurum]
  FullString(String),
  JustNumber(u32),
}

struct Echoer {
  confirm_start: Sender<()>,
  echo_recvr: Sender<Echo>,
}
#[async_trait]
impl Actor<RegTestTypes, Echo> for Echoer {
  async fn pre_start(&mut self, _: &ActorContext<RegTestTypes, Echo>) {
    self.confirm_start.send(()).unwrap();
  }
  async fn recv(&mut self, _ctx: &ActorContext<RegTestTypes, Echo>, msg: Echo) {
    self.echo_recvr.send(msg).unwrap();
  }
}

unify!(RegTestTypes = Echo | String);

#[test]
fn registry_test() {
  let node = Node::<RegTestTypes>::new(
    Socket::new(Host::DNS("localhost".to_string()), 1000, 1001),
    1,
  );
  let echo_name = ActorName::new::<Echo>("echoer".to_string());
  let (confirm_tx, confirm_rx) = bounded(1);
  let (tx, rx) = unbounded();
  node.spawn(
    false,
    Echoer {
      confirm_start: confirm_tx,
      echo_recvr: tx,
    },
    "echoer".to_string(),
    true,
  );
  let str_ser = serialize(LocalActorMsg::Msg("oh no!".to_string())).unwrap();
  confirm_rx.recv().unwrap();
  node.registry(RegistryMsg::Forward(
    echo_name,
    <RegTestTypes as Case<String>>::VARIANT,
    str_ser,
  ));
  match rx.recv() {
    Ok(echo) => assert_eq!(echo, Echo::FullString("oh no!".to_string())),
    Err(_) => panic!("Could not receive!"),
  }
}
