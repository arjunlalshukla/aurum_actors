use crate::core::{Actor, ActorContext, Address, Case, HiddenInterface, SpecificInterface};
use crate::core::LocalRef;
use std::fmt::Debug;
use std::marker::Send;

pub fn spawn<Unified, Specific, A>(actor: A, addr: Address<Unified>) 
 -> LocalRef<Specific> where 
 A: Actor<Unified, Specific> + Send + 'static, 
 Specific: 'static + Send + SpecificInterface<Unified>,
 Unified: Clone + Case<Specific> + 'static + Send + Debug {
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

