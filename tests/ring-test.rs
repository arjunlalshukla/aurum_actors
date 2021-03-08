use async_trait::async_trait;
use aurum::core::{
  Actor, ActorContext, ActorName, ActorSignal, Host, LocalRef, Node, Socket,
};
use aurum_macros::{unify, AurumInterface};
use crossbeam::channel::{unbounded, Sender};

const ROUNDS: u32 = 100;
const RING_SIZE: u16 = 20;

#[derive(AurumInterface, Clone)]
#[aurum(local)]
enum Ball {
  Ball(u32, ActorName<RingTypes>),
}
unify!(RingTypes = Ball);

#[derive(PartialEq, Eq, Debug)]
enum TestRecvr {
  IntraRing(u32, ActorName<RingTypes>),
  IAmDying(ActorName<RingTypes>),
}

struct Player {
  tester: Sender<TestRecvr>,
  ring_num: u16,
  next: LocalRef<Ball>,
  leader: Option<LocalRef<Ball>>,
  double: bool,
  register: bool,
}
#[async_trait]
impl Actor<RingTypes, Ball> for Player {
  async fn pre_start(&mut self, ctx: &ActorContext<RingTypes, Ball>) {
    if self.ring_num != 0 {
      self.next = ctx.node.spawn(
        self.double,
        Player {
          tester: self.tester.clone(),
          ring_num: self.ring_num - 1,
          next: LocalRef::panic(),
          leader: Some(self.leader.clone().unwrap_or(ctx.local_interface())),
          double: self.double,
          register: self.register,
        },
        format!("ring-member-{}", self.ring_num - 1),
        self.register,
      );
    } else {
      self.next = self.leader.clone().unwrap_or(ctx.local_interface());
    }
    if self.leader.is_none() {
      self.next.send(Ball::Ball(0, ctx.name.clone()));
    }
  }

  async fn recv(&mut self, ctx: &ActorContext<RingTypes, Ball>, msg: Ball) {
    let Ball::Ball(hit_num, sender) = msg;
    self
      .tester
      .send(TestRecvr::IntraRing(hit_num, sender))
      .unwrap();
    if hit_num < (RING_SIZE) as u32 * ROUNDS - 1 {
      self.next.send(Ball::Ball(hit_num + 1, ctx.name.clone()));
    }
  }

  async fn post_stop(&mut self, ctx: &ActorContext<RingTypes, Ball>) {
    if self.ring_num != 0 {
      if !self.next.signal(ActorSignal::Term) {
        panic!("{:?} could not send kill message to next", ctx.name);
      }
    }
    self
      .tester
      .send(TestRecvr::IAmDying(ctx.name.clone()))
      .unwrap();
  }
}

fn ring_test(double: bool, register: bool) {
  let (tx, rx) = unbounded();
  let node = Node::<RingTypes>::new(
    Socket::new(Host::DNS("localhost".to_string()), 1000, 1001),
    1,
  )
  .unwrap();
  let names = (0..RING_SIZE)
    .rev()
    .map(|x| ActorName::<RingTypes>::new::<Ball>(format!("ring-member-{}", x)))
    .collect::<Vec<_>>();
  let leader = node.spawn(
    double,
    Player {
      tester: tx,
      ring_num: RING_SIZE - 1,
      next: LocalRef::panic(),
      leader: None,
      double: double,
      register: register,
    },
    format!("ring-member-{}", RING_SIZE - 1),
    register,
  );
  for x in 0..ROUNDS {
    for (name, num) in names.iter().zip(0..RING_SIZE) {
      match rx.recv() {
        Err(_) => panic!("round = {}; name = {:?}", x, name),
        Ok(res) => {
          assert_eq!(
            res,
            TestRecvr::IntraRing(
              x * RING_SIZE as u32 + num as u32,
              name.clone()
            )
          );
        }
      }
    }
  }
  leader.signal(ActorSignal::Term);
  for name in names {
    assert_eq!(rx.recv(), Ok(TestRecvr::IAmDying(name.clone())));
  }
}

#[test]
fn ring_single_registered() {
  ring_test(false, true);
}
#[test]
fn ring_single_unregistered() {
  ring_test(false, false);
}
#[test]
fn ring_double_registered() {
  ring_test(true, true);
}
#[test]
fn ring_double_unregistered() {
  ring_test(true, false);
}
