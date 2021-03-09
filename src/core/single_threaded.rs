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
  A: Actor<U, S> + Send + 'static,
  S: 'static + Send + SpecificInterface<U>,
  U: UnifiedBounds + Case<S>,
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
      ActorMsg::PrimaryRequest => {
        panic!("{:?} single got a primary request", ctx.name)
      }
      ActorMsg::Die => {
        panic!("A single threaded actor shouldn't get ActorMsg::Die")
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
