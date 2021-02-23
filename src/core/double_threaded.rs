use crate::core::{
  Actor, ActorContext, ActorMsg, ActorName, Case, LocalActorMsg, Node,
  RegistryMsg, SpecificInterface, UnifiedBounds,
};
use std::collections::VecDeque;
use tokio::sync::mpsc::{
  unbounded_channel, UnboundedReceiver, UnboundedSender,
};
use tokio::sync::oneshot::channel;

enum PrimaryMsg<Specific> {
  Msg(Specific),
  Die,
}

pub(crate) async fn run_secondary<Specific, A, Unified>(
  node: Node<Unified>,
  actor: A,
  name: String,
  tx: UnboundedSender<ActorMsg<Unified, Specific>>,
  mut rx: UnboundedReceiver<ActorMsg<Unified, Specific>>,
  register: bool,
) where
  A: Actor<Unified, Specific> + Send + 'static,
  Specific: 'static + Send + SpecificInterface<Unified>,
  Unified: UnifiedBounds + Case<RegistryMsg<Unified>> + Case<Specific>,
{
  let name = ActorName::new::<Specific>(name);
  let ctx = ActorContext {
    tx: tx,
    name: name.clone(),
    node: node.clone(),
  };
  if register {
    let (tx, rx) = channel::<()>();
    node.registry(RegistryMsg::Register(name.clone(), ctx.ser_recvr(), tx));
    match rx.await {
      Err(_) => panic!("Could not register {:?}", name),
      _ => (),
    };
  }
  let mut queue = VecDeque::<PrimaryMsg<Specific>>::new();
  let mut primary_waiting = false;
  let (primary_tx, primary_rx) = unbounded_channel::<PrimaryMsg<Specific>>();
  node.node.rt.spawn(run_primary(actor, ctx, primary_rx));
  let send_to_primary = |msg: PrimaryMsg<Specific>| {
    if primary_tx.send(msg).is_err() {
      panic!("{:?} lost connection with primary", name);
    }
  };
  loop {
    let msg = match rx.recv().await.unwrap() {
      ActorMsg::Msg(x) => x,
      ActorMsg::Die => break,
      ActorMsg::PrimaryRequest => {
        if primary_waiting {
          panic!("{:?} single got a primary request", name);
        } else {
          match queue.pop_front() {
            Some(msg) => send_to_primary(msg),
            None => primary_waiting = true,
          }
          continue;
        }
      }
      ActorMsg::Serial(interface, bytes) => {
        <Specific as SpecificInterface<Unified>>::deserialize_as(
          interface, bytes,
        )
        .unwrap()
      }
    };
    let pri = match msg {
      LocalActorMsg::Msg(lam) => Some(PrimaryMsg::Msg(lam)),
      LocalActorMsg::EagerKill => Some(PrimaryMsg::Die),
    };
    for pri in pri.into_iter() {
      if primary_waiting {
        send_to_primary(pri);
        primary_waiting = false;
      } else {
        queue.push_back(pri);
      }
    }
  }
  if register {
    node.registry(RegistryMsg::Deregister(name));
  }
}

async fn run_primary<Specific, A, Unified>(
  mut actor: A,
  ctx: ActorContext<Unified, Specific>,
  mut rx: UnboundedReceiver<PrimaryMsg<Specific>>,
) where
  A: Actor<Unified, Specific> + Send + 'static,
  Specific: 'static + Send + SpecificInterface<Unified>,
  Unified: Case<Specific>,
{
  let send_to_secondary = |x: ActorMsg<Unified, Specific>| {
    if ctx.tx.send(x).is_err() {
      panic!("{:?}: secondary is unreachable", ctx.name);
    }
  };
  actor.pre_start(&ctx).await;
  loop {
    send_to_secondary(ActorMsg::PrimaryRequest);
    let msg: PrimaryMsg<Specific> = match rx.recv().await {
      None => panic!("{:?}: the secondary seems to have crashed", ctx.name),
      Some(x) => x,
    };
    match msg {
      PrimaryMsg::Die => break,
      PrimaryMsg::Msg(m) => actor.recv(&ctx, m).await,
    }
  }
  actor.post_stop(&ctx).await;
  send_to_secondary(ActorMsg::Die);
}
