use async_trait::async_trait;
use aurum::core::{
  Actor, ActorContext, ActorName, Host, LocalRef, Node, Socket,
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

struct Player {
  tester: Sender<(u32, ActorName<RingTypes>)>,
  ring_num: u16,
  next: LocalRef<Ball>,
  leader: Option<LocalRef<Ball>>,
}
#[async_trait]
impl Actor<RingTypes, Ball> for Player {
  async fn pre_start(&mut self, ctx: &ActorContext<RingTypes, Ball>) {
    if self.ring_num != 0 {
      self.next = ctx.node.spawn_local_single(
        Player {
          tester: self.tester.clone(),
          ring_num: self.ring_num - 1,
          next: LocalRef::panic(),
          leader: Some(self.leader.clone().unwrap_or(ctx.local_interface())),
        },
        format!("ring-member-{}", self.ring_num - 1),
        true,
      );
    } else {
      self.next = self.leader.clone().unwrap_or(ctx.local_interface());
    }
    if self.leader.is_none() {
      self.next.send(Ball::Ball(0, ctx.name.clone()));
    }
  }

  async fn recv(&mut self, ctx: &ActorContext<RingTypes, Ball>, msg: Ball) {
    match msg {
      Ball::Ball(hit_num, sender) => {
        self.tester.send((hit_num, sender)).unwrap();
        if hit_num < (RING_SIZE) as u32 * ROUNDS {
          self.next.send(Ball::Ball(hit_num + 1, ctx.name.clone()));
        }
      }
    }
  }
}

#[test]
fn ring() {
  let (tx, rx) = unbounded();
  let node = Node::<RingTypes>::new(
    Socket::new(Host::DNS("localhost".to_string()), 1000, 1001),
    1,
  );
  let names = (0..RING_SIZE)
    .rev()
    .map(|x| ActorName::<RingTypes>::new::<Ball>(format!("ring-member-{}", x)))
    .collect::<Vec<_>>();
  node.spawn_local_single(
    Player {
      tester: tx,
      ring_num: RING_SIZE - 1,
      next: LocalRef::panic(),
      leader: None,
    },
    format!("ring-member-{}", RING_SIZE - 1),
    true,
  );
  for x in 0..ROUNDS {
    //println!("Running round x");
    for (name, num) in names.iter().zip(0..RING_SIZE) {
      match rx.recv() {
        Err(_) => panic!("round = {}; name = {:?}", x, name),
        Ok((hit_num, sender)) => {
          assert_eq!(hit_num, x * RING_SIZE as u32 + num as u32);
          assert_eq!(&sender, name);
        }
      }
    }
  }
}
