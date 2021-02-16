#![allow(dead_code)]
#![allow(unused_variables)]
use crate::core::{
  Actor, ActorContext, ActorMsg, ActorName, Case, LocalRef, RegistryMsg,
  Socket, SpecificInterface, UnifiedBounds,
};
use crossbeam::channel::{Receiver, Sender};
use std::sync::Arc;

pub struct Node<Unified: UnifiedBounds> {
  pub socket: Socket,
  registry: LocalRef<RegistryMsg<Unified>>,
}
impl<Unified: UnifiedBounds> Node<Unified> {
  pub fn new(socket: Socket) {}

  fn start_codependent<Specific, A>() {}

  fn run_local<Specific, A>(
    node: Arc<Node<Unified>>,
    mut actor: A,
    name: ActorName<Unified>,
    tx: Sender<ActorMsg<Unified, Specific>>,
    rx: Receiver<ActorMsg<Unified, Specific>>,
  ) where
    A: Actor<Unified, Specific> + Send + 'static,
    Specific: 'static + Send + SpecificInterface<Unified>,
    Unified: Case<Specific>,
  {
    let ctx = ActorContext {
      tx: tx.clone(),
      name: name,
      node: node,
    };
    let lcl_ref = ctx.local_interface::<Specific>();
    loop {
      let recvd = rx.recv().unwrap();
      let msg: Specific = match recvd {
        ActorMsg::Msg(x) => x,
        ActorMsg::Serial(interface, bytes) => {
          <Specific as SpecificInterface<Unified>>::deserialize_as(
            interface, bytes,
          )
          .unwrap()
        }
      };
      actor.recv(&ctx, msg)
    }
  }
}
