#![allow(dead_code)]
#![allow(unused_variables)]
use crate::core::{
  Actor, ActorContext, Address, Case, HiddenInterface, LocalRef, RegistryMsg, 
  Socket, SpecificInterface
};
use std::fmt::Debug;

pub struct Node<Unified> {
  socket: Socket,
  registry: LocalRef<RegistryMsg<Unified>>
}
impl<Unified: 'static + Send + Debug + Clone> Node<Unified> {
  pub fn new(socket: Socket) {
    
  }

  pub fn spawn_local<Specific, A>(&self, actor: A, addr: Address<Unified>) 
   -> LocalRef<Specific> where 
   A: Actor<Unified, Specific> + Send + 'static, 
   Specific: 'static + Send + SpecificInterface<Unified>,
   Unified: Case<Specific> {
    let (tx, rx) = 
      crossbeam::channel::unbounded::<HiddenInterface<Unified, Specific>>();
    let ctx = ActorContext {tx: tx.clone(), address: addr};
    let lcl_ref = ctx.local_ref::<Specific>();
    std::thread::spawn(move || {
      let mut actor = actor;
      loop {
        let recvd = rx.recv().unwrap();
        let msg: Specific = match recvd {
          HiddenInterface::Msg(x) => x,
          HiddenInterface::Serial(interface, bytes) => 
            <Specific as SpecificInterface<Unified>>::deserialize_as(interface, bytes).unwrap()
        };
        actor.recv(&ctx, msg)
      }
    });
    lcl_ref
  }
}