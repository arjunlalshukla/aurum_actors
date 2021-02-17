use aurum::core::{
  serialize, Actor, ActorContext, Host, Node, RegistryMsg, Socket,
};
use aurum::{
  core::{ActorName, Case},
  unified,
};
use crossbeam::channel::{bounded, unbounded, Sender};
use interface_proc::AurumInterface;
use serde::{Deserialize, Serialize};

type RegType = RegistryMsg<RegTestTypes>;

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
impl Actor<RegTestTypes, Echo> for Echoer {
  fn pre_start(&mut self) {
    self.confirm_start.send(()).unwrap();
  }
  fn recv(&mut self, _ctx: &ActorContext<RegTestTypes, Echo>, msg: Echo) {
    self.echo_recvr.send(msg).unwrap();
  }
}

unified! {RegTestTypes = Echo | RegType | String}

#[test]
fn registry_test() {
  let node = Node::<RegTestTypes>::new(Socket::new(
    Host::DNS("localhost".to_string()),
    1000,
    1001,
  ));
  let echo_name = ActorName::new::<Echo>("echoer".to_string());
  let (confirm_tx, confirm_rx) = bounded(1);
  let (tx, rx) = unbounded();
  node.spawn(
    Echoer {
      confirm_start: confirm_tx,
      echo_recvr: tx,
    },
    echo_name.clone(),
    true,
  );
  let str_ser = serialize("oh no!".to_string()).unwrap();
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
