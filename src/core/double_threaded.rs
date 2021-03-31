use crate::core::{
  Actor, ActorContext, ActorMsg, ActorSignal, Case, LocalActorMsg, RegistryMsg,
  SpecificInterface, UnifiedBounds,
};
use std::collections::VecDeque;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
use tokio::sync::oneshot::channel;

enum PrimaryMsg<S> {
  Msg(S),
  Die,
}

pub(crate) async fn run_secondary<U, S, A>(
  actor: A,
  ctx: ActorContext<U, S>,
  mut rx: UnboundedReceiver<ActorMsg<U, S>>,
  register: bool,
) where
  U: UnifiedBounds + Case<S>,
  S: 'static + Send + SpecificInterface<U>,
  A: Actor<U, S> + Send + 'static,
{
  if register {
    let (tx, rx) = channel::<()>();
    ctx.node.registry(RegistryMsg::Register(
      ctx.name.clone(),
      ctx.ser_recvr(),
      tx,
    ));
    rx.await
      .expect(format!("Could not register {:?}", ctx.name).as_str());
  }
  let mut queue = VecDeque::<PrimaryMsg<S>>::new();
  let mut primary_waiting = false;
  let (primary_tx, primary_rx) = unbounded_channel::<PrimaryMsg<S>>();
  let node = ctx.node.clone();
  let name = ctx.name.clone();
  node.rt().spawn(run_primary(actor, ctx, primary_rx));
  let send_to_primary = |msg: PrimaryMsg<S>| {
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
      ActorMsg::Serial(interface, mb) => {
        <S as SpecificInterface<U>>::deserialize_as(
          interface,
          mb.intp,
          mb.msg(),
        )
        .unwrap()
      }
    };
    let pri = match msg {
      LocalActorMsg::Msg(lam) => Some(PrimaryMsg::Msg(lam)),
      LocalActorMsg::Signal(ActorSignal::Term) => Some(PrimaryMsg::Die),
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

async fn run_primary<U, S, A>(
  mut actor: A,
  ctx: ActorContext<U, S>,
  mut rx: UnboundedReceiver<PrimaryMsg<S>>,
) where
  U: UnifiedBounds + Case<S>,
  S: 'static + Send + SpecificInterface<U>,
  A: Actor<U, S> + Send + 'static,
{
  let send_to_secondary = |x: ActorMsg<U, S>| {
    if ctx.tx.send(x).is_err() {
      panic!("{:?}: secondary is unreachable", ctx.name);
    }
  };
  actor.pre_start(&ctx).await;
  loop {
    send_to_secondary(ActorMsg::PrimaryRequest);
    let msg: PrimaryMsg<S> = match rx.recv().await {
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
