use crate::core::{
  Actor, ActorContext, ActorMsg, ActorName, Case, LocalRef, Registry,
  RegistryMsg, Socket, SpecificInterface, UnifiedBounds,
};
use crossbeam::channel::{unbounded, Receiver, Sender};
use std::sync::Arc;

pub struct Node<Unified: UnifiedBounds> {
  pub socket: Socket,
  pub registry: LocalRef<RegistryMsg<Unified>>,
}
impl<Unified: UnifiedBounds + Case<RegistryMsg<Unified>>> Node<Unified> {
  pub fn new(socket: Socket) -> Arc<Node<Unified>> {
    let (reg, reg_node_tx) = Self::start_codependent(
      Registry::new(),
      ActorName::new::<RegistryMsg<Unified>>("registry".to_string()),
    );
    let node = Arc::new(Node {
      socket: socket,
      registry: reg,
    });
    if reg_node_tx.send(node.clone()).is_err() {
      panic!("Could not send node to registry");
    }
    node
  }

  fn start_codependent<Specific, A>(
    actor: A,
    name: ActorName<Unified>,
  ) -> (LocalRef<Specific>, Sender<Arc<Node<Unified>>>)
  where
    A: Actor<Unified, Specific> + Send + 'static,
    Specific: 'static + Send + SpecificInterface<Unified>,
    Unified: Case<Specific>,
  {
    let (tx, rx) = unbounded::<ActorMsg<Unified, Specific>>();
    let (node_tx, node_rx) = unbounded::<Arc<Node<Unified>>>();
    let ret = (
      ActorContext::<Unified, Specific>::create_local(tx.clone()),
      node_tx,
    );
    std::thread::spawn(move || {
      Self::run_single(node_rx.recv().unwrap(), actor, name, tx, rx, true)
    });
    ret
  }

  fn run_single<Specific, A>(
    node: Arc<Node<Unified>>,
    mut actor: A,
    name: ActorName<Unified>,
    tx: Sender<ActorMsg<Unified, Specific>>,
    rx: Receiver<ActorMsg<Unified, Specific>>,
    register: bool,
  ) where
    A: Actor<Unified, Specific> + Send + 'static,
    Specific: 'static + Send + SpecificInterface<Unified>,
    Unified: Case<Specific>,
  {
    let ctx = ActorContext {
      tx: tx,
      name: name.clone(),
      node: node,
    };
    if register {
      (&ctx.node.registry)(RegistryMsg::Register(name, ctx.ser_recvr()));
    }
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

  pub fn spawn<Specific, A>(
    node: Arc<Node<Unified>>,
    actor: A,
    name: ActorName<Unified>,
    register: bool,
  ) -> LocalRef<Specific>
  where
    A: Actor<Unified, Specific> + Send + 'static,
    Specific: 'static + Send + SpecificInterface<Unified>,
    Unified: Case<Specific>,
  {
    let (tx, rx) = unbounded::<ActorMsg<Unified, Specific>>();
    let ret = ActorContext::<Unified, Specific>::create_local(tx.clone());
    std::thread::spawn(move || {
      Self::run_single(node, actor, name, tx, rx, register)
    });
    ret
  }
}
