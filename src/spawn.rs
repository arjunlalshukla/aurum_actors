use crate::actor::{Actor, ActorContext, Address, HiddenInterface, SpecificInterface};
use crate::unify::Case;
use crate::actor::LocalRef;
use std::fmt::Debug;
use std::marker::Send;

fn spawn<Unified, Specific, A>(actor: A, name: String, addr: Address<Unified>) 
 -> LocalRef<Specific> where 
 A: Actor<Unified, Specific>, 
 Specific: 'static + Send + SpecificInterface<Unified>,
 Unified: Clone + Case<Specific> + 'static + Send + Debug {
  let (tx, rx) = 
    crossbeam::channel::unbounded::<HiddenInterface<Unified, Specific>>();
  let ctx = ActorContext {tx: tx.clone(), address: addr};
  let lcl_ref = ctx.local_ref::<Specific>();
  std::thread::spawn(move || loop {
    let recvd = rx.recv().unwrap();
    let msg: Specific = match recvd {
      HiddenInterface::Msg(x) => x,
      HiddenInterface::Serial(interface, bytes) => 
        <Specific as SpecificInterface<Unified>>::deserialize_as(interface, bytes).unwrap()
    };
  });
  lcl_ref
}

