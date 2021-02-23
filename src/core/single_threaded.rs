use crate::core::{
  Actor, ActorContext, ActorMsg, ActorName, Case, LocalActorMsg, Node,
  RegistryMsg, SpecificInterface, UnifiedBounds,
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot::channel;

pub(crate) async fn run_single<Unified, Specific, A>(
  node: Node<Unified>,
  mut actor: A,
  name: String,
  tx: UnboundedSender<ActorMsg<Unified, Specific>>,
  mut rx: UnboundedReceiver<ActorMsg<Unified, Specific>>,
  register: bool,
) where
  A: Actor<Unified, Specific> + Send + 'static,
  Specific: 'static + Send + SpecificInterface<Unified>,
  Unified: Case<RegistryMsg<Unified>> + UnifiedBounds + Case<Specific>,
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
      ActorMsg::Serial(interface, bytes) => {
        <Specific as SpecificInterface<Unified>>::deserialize_as(
          interface, bytes,
        )
        .unwrap()
      }
    };
    match msg {
      LocalActorMsg::Msg(m) => actor.recv(&ctx, m).await,
      LocalActorMsg::EagerKill => break,
    };
  }
  actor.post_stop(&ctx).await;
  if register {
    node.registry(RegistryMsg::Deregister(name));
  }
}
