#![allow(dead_code)]
#![allow(unused_variables)]
use crossbeam::channel::{Sender, Receiver};
use crate::core::{
  Actor, ActorContext, Address, Case, ActorMsg, LocalRef, RegistryMsg, UnifiedBounds,
  Socket, SpecificInterface
};
use std::sync::Arc;

pub struct Node<Unified: UnifiedBounds> {
  socket: Socket,
  registry: LocalRef<RegistryMsg<Unified>>
}
impl<Unified: UnifiedBounds> Node<Unified> {
  pub fn new(socket: Socket) {
    
  }

  fn start_codependent<Specific, A>() {
    
  }

  fn run_local<Specific, A>(
    node: Arc<Node<Unified>>, 
    mut actor: A, 
    addr: Address<Unified>,
    tx: Sender<ActorMsg<Unified, Specific>>,
    rx: Receiver<ActorMsg<Unified, Specific>>
  ) where 
    A: Actor<Unified, Specific> + Send + 'static, 
    Specific: 'static + Send + SpecificInterface<Unified>,
    Unified: Case<Specific>
  {
    let ctx = ActorContext {tx: tx.clone(), address: addr};
    let lcl_ref = ctx.local_ref::<Specific>();
    loop {
      let recvd = rx.recv().unwrap();
      let msg: Specific = match recvd {
        ActorMsg::Msg(x) => x,
        ActorMsg::Serial(interface, bytes) => 
          <Specific as SpecificInterface<Unified>>::deserialize_as(interface, bytes).unwrap()
      };
      actor.recv(&ctx, msg)
    }
  }
}