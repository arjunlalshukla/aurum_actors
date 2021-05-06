use crate::core::{
  ActorContext, ActorMsg, ActorSignal, Case, LocalActorMsg, RegistryMsg,
  SpecificInterface, TimeoutActor, UnifiedType,
};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::oneshot::channel;

pub(crate) async fn run_single_timeout<U, S, A>(
  mut actor: A,
  ctx: ActorContext<U, S>,
  mut rx: UnboundedReceiver<ActorMsg<U, S>>,
  register: bool,
  mut timeout: Duration,
) where
  U: UnifiedType + Case<S>,
  S: 'static + Send + SpecificInterface<U>,
  A: TimeoutActor<U, S> + Send + 'static,
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
  timeout = actor.pre_start(&ctx).await.unwrap_or(timeout);
  loop {
    tokio::select! {
      msg = rx.recv() => {
        let msg = match msg.unwrap() {
          ActorMsg::Msg(x) => x,
          ActorMsg::Serial(interface, mb) => {
            S::deserialize_as(interface, mb.intp, mb.msg()).unwrap()
          }
          _ => unreachable!()
        };
        match msg {
          LocalActorMsg::Msg(m) => {
            timeout = actor.recv(&ctx, m).await.unwrap_or(timeout);
          }
          LocalActorMsg::Signal(ActorSignal::Term) => break,
        };
      }
      _ = tokio::time::sleep(timeout) => {
        timeout = actor.timeout(&ctx).await.unwrap_or(timeout);
      }
    }
  }
  actor.post_stop(&ctx).await;
  if register {
    ctx.node.registry(RegistryMsg::Deregister(ctx.name));
  }
}
