use crate::core::{
  Actor, ActorContext, ActorMsg, ActorSignal, Case, LocalActorMsg, RegistryMsg,
  SpecificInterface, UnifiedBounds,
};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::oneshot::channel;

pub(crate) async fn run_single<U, S, A>(
  mut actor: A,
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
  actor.pre_start(&ctx).await;
  loop {
    let msg = match rx.recv().await.unwrap() {
      ActorMsg::Msg(x) => x,
      ActorMsg::Serial(interface, mb) => {
        S::deserialize_as(interface, mb.intp, mb.msg()).unwrap()
      }
      _ => unreachable!(),
    };
    match msg {
      LocalActorMsg::Msg(m) => actor.recv(&ctx, m).await,
      LocalActorMsg::Signal(ActorSignal::Term) => break,
    };
  }
  actor.post_stop(&ctx).await;
  if register {
    ctx.node.registry(RegistryMsg::Deregister(ctx.name));
  }
}
