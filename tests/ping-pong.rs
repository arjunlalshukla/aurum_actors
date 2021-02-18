use std::u32;

use aurum::core::{Actor, ActorContext, ActorName, Host, LocalRef, Node, RegistryMsg, Socket};
use aurum::unified;
use crossbeam::channel::{unbounded, Sender};
use aurum_macros::AurumInterface;

const RALLIES: u32 = 100;

#[derive(Debug, PartialEq, Eq)]
enum Hit {
  Ping(u32),
  Pong(u32)
}

type Reg = RegistryMsg<MsgTypes>;


#[derive(AurumInterface)]
#[aurum(local)]
enum Ball {
  Ball(Hit, LocalRef<Ball>)
}
unified!(MsgTypes = Ball | Reg);

struct Player {
  tester: Sender<(Hit, ActorName<MsgTypes>)>,
  pinger: bool
}
impl Actor<MsgTypes, Ball> for Player {
    fn pre_start(&mut self, ctx: &ActorContext<MsgTypes, Ball>) {
      if self.pinger {
        let r = ctx.node.spawn_local_single(
          Player { tester: self.tester.clone(), pinger: false}, 
          "ponger".to_string(), 
          true
        );
        r(Ball::Ball(Hit::Ping(0), ctx.local_interface()));
      }
    }

    fn recv(&mut self, ctx: &ActorContext<MsgTypes, Ball>, msg: Ball) {
      match msg {
        Ball::Ball(Hit::Ping(x), r) => {
          self.tester.send((Hit::Ping(x), ctx.name.clone())).unwrap();
          r(Ball::Ball(Hit::Pong(x+1), ctx.local_interface()));
        }
        Ball::Ball(Hit::Pong(x), r) => {
          self.tester.send((Hit::Pong(x), ctx.name.clone())).unwrap();
          if x <= 2*RALLIES {
            r(Ball::Ball(Hit::Ping(x+1), ctx.local_interface()));
          }
        }
      }
    }
}

#[test]
fn ping_pong() {
  let (tx, rx) = unbounded();
  let node = Node::<MsgTypes>::new(Socket::new(
    Host::DNS("localhost".to_string()),
    1000,
    1001,
  ));
  let ping = ActorName::<MsgTypes>::new::<Ball>("pinger".to_string());
  let pong = ActorName::<MsgTypes>::new::<Ball>("ponger".to_string());
  node.spawn_local_single(Player{tester: tx, pinger: true}, "pinger".to_string(), true);
  for x in 0..RALLIES {
    assert_eq!(rx.recv(), Ok((Hit::Ping(2*x), pong.clone())));
    assert_eq!(rx.recv(), Ok((Hit::Pong(2*x+1), ping.clone())));
  }
}