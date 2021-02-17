use aurum::core::{
  serialize, Actor, ActorContext, Host, Node, RegistryMsg, Socket,
};
use aurum::{
  core::{ActorName, Case},
  unified,
};
use interface_proc::AurumInterface;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

type RegType = RegistryMsg<RegTestTypes>;

#[derive(AurumInterface, Serialize, Deserialize)]
enum Echo {
  #[aurum]
  FullString(String),
  JustNumber(u32),
}

struct Echoer {}
impl Actor<RegTestTypes, Echo> for Echoer {
  fn recv(&mut self, _ctx: &ActorContext<RegTestTypes, Echo>, msg: Echo) {
    match msg {
      Echo::FullString(s) => println!("Got String: \"{}\"", s),
      Echo::JustNumber(x) => println!("Got number: \"{}\"", x),
    }
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
  Node::spawn(node.clone(), Echoer {}, echo_name.clone(), true);
  let str_ser = serialize("oh no!".to_string()).unwrap();
  std::thread::sleep_ms(1000);
  (&node.registry)(RegistryMsg::Forward(
    echo_name,
    <RegTestTypes as Case<String>>::VARIANT,
    str_ser,
  ));
  std::thread::sleep_ms(5000);
}
