use crate::core::{
  Actor, ActorContext, ActorMsg, ActorName, Case, LocalRef, Registry,
  RegistryMsg, Socket, SpecificInterface, UnifiedBounds,
};
use crossbeam::channel::{bounded, unbounded, Receiver, Sender};
use std::sync::Arc;

struct NodeImpl<Unified: UnifiedBounds> {
  socket: Socket,
  registry: LocalRef<RegistryMsg<Unified>>,
}

#[derive(Clone)]
pub struct Node<Unified: UnifiedBounds> {
  node: Arc<NodeImpl<Unified>>,
}
impl<Unified: UnifiedBounds + Case<RegistryMsg<Unified>>> Node<Unified> {
  pub fn new(socket: Socket) -> Self {
    let (reg, reg_node_tx) = Self::start_codependent(
      Registry::new(),
      ActorName::new::<RegistryMsg<Unified>>("registry".to_string()),
    );
    let node = Node {
      node: Arc::new(NodeImpl {
        socket: socket,
        registry: reg,
      }),
    };
    if reg_node_tx.send(node.clone()).is_err() {
      panic!("Could not send node to registry");
    }
    node
  }

  pub fn socket(&self) -> &Socket {
    &self.node.socket
  }

  pub fn registry(&self, msg: RegistryMsg<Unified>) {
    (&self.node.registry)(msg);
  }

  pub fn spawn<Specific, A>(
    &self,
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
    let node = self.clone();
    std::thread::spawn(move || node.run_single(actor, name, tx, rx, register));
    ret
  }



  fn start_codependent<Specific, A>(
    actor: A,
    name: ActorName<Unified>,
  ) -> (LocalRef<Specific>, Sender<Self>)
  where
    A: Actor<Unified, Specific> + Send + 'static,
    Specific: 'static + Send + SpecificInterface<Unified>,
    Unified: Case<Specific>,
  {
    let (tx, rx) = unbounded::<ActorMsg<Unified, Specific>>();
    let (node_tx, node_rx) = unbounded::<Node<Unified>>();
    let ret = (
      ActorContext::<Unified, Specific>::create_local(tx.clone()),
      node_tx,
    );
    std::thread::spawn(move || {
      node_rx
        .recv()
        .unwrap()
        .run_single(actor, name, tx, rx, false)
    });
    ret
  }


  
  fn run_single<Specific, A>(
    self,
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
      node: self.clone(),
    };
    if register {
      let (tx, rx) = bounded::<()>(1);
      self.registry(RegistryMsg::Register(name.clone(), ctx.ser_recvr(), tx));
      match rx.recv() {
        Err(_) => panic!("Could not register {:?}", name),
        _ => (),
      };
    }
    actor.pre_start();
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
