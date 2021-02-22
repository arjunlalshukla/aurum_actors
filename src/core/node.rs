use crate::core::{
  Actor, ActorContext, ActorMsg, ActorName, Case, LocalRef, Registry,
  RegistryMsg, Socket, SpecificInterface, UnifiedBounds,
};
use std::sync::Arc;
use tokio::runtime::{Builder, Runtime};
use tokio::sync::oneshot::channel;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender, UnboundedReceiver};

struct NodeImpl<Unified: UnifiedBounds> {
  socket: Socket,
  registry: LocalRef<RegistryMsg<Unified>>,
  rt: Runtime
}

#[derive(Clone)]
pub struct Node<Unified: UnifiedBounds> {
  node: Arc<NodeImpl<Unified>>,
}
impl<Unified: UnifiedBounds + Case<RegistryMsg<Unified>>> Node<Unified> {
  pub fn new(socket: Socket, actor_threads: usize) -> Self {
    let rt = Builder::new_multi_thread()
    .worker_threads(actor_threads)
    .thread_name("tokio-thread")
    .thread_stack_size(3 * 1024 * 1024)
    .build()
    .unwrap();
    let (reg, reg_node_tx) =
      Self::start_codependent(&rt, Registry::new(), "registry".to_string());
    let node = Node {
      node: Arc::new(NodeImpl {
        socket: socket,
        registry: reg,
        rt: rt
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

  pub fn registry(&self, msg: RegistryMsg<Unified>) -> bool {
    self.node.registry.send(msg)
  }



  pub fn spawn_local_single<Specific, A>(
    &self,
    actor: A,
    name: String,
    register: bool,
  ) -> LocalRef<Specific>
  where
    A: Actor<Unified, Specific> + Send + 'static,
    Specific: 'static + Send + SpecificInterface<Unified>,
    Unified: Case<Specific>,
  {
    let (tx, rx) = unbounded_channel::<ActorMsg<Unified, Specific>>();
    let ret = ActorContext::<Unified, Specific>::create_local(tx.clone());
    let node = self.clone();
    self.node.rt.spawn(node.run_single(actor, name, tx, rx, register));
    ret
  }



  fn start_codependent<Specific, A>(
    rt: &Runtime,
    actor: A,
    name: String,
  ) -> (LocalRef<Specific>, UnboundedSender<Self>)
  where
    A: Actor<Unified, Specific> + Send + 'static,
    Specific: 'static + Send + SpecificInterface<Unified>,
    Unified: Case<Specific>,
  {
    let (tx, rx) = unbounded_channel::<ActorMsg<Unified, Specific>>();
    let (node_tx, mut node_rx) = unbounded_channel::<Node<Unified>>();
    let ret = (
      ActorContext::<Unified, Specific>::create_local(tx.clone()),
      node_tx,
    );
    rt.spawn(async move {
      node_rx.recv().await.unwrap().run_single(actor, name, tx, rx, false).await
    });
    ret
  }



  async fn run_single<Specific, A>(
    self,
    mut actor: A,
    name: String,
    tx: UnboundedSender<ActorMsg<Unified, Specific>>,
    mut rx: UnboundedReceiver<ActorMsg<Unified, Specific>>,
    register: bool,
  ) where
    A: Actor<Unified, Specific> + Send + 'static,
    Specific: 'static + Send + SpecificInterface<Unified>,
    Unified: Case<Specific>,
  {
    let name = ActorName::new::<Specific>(name);
    let ctx = ActorContext {
      tx: tx,
      name: name.clone(),
      node: self.clone(),
    };
    if register {
      let (tx, rx) = channel::<()>();
      if !self.registry(RegistryMsg::Register(name.clone(), ctx.ser_recvr(), tx)) {
        println!("Could not send to registry");
      } 
      match rx.await {
        Err(_) => panic!("Could not register {:?}", name),
        _ => (),
      };
    }
    actor.pre_start(&ctx).await;
    loop {
      let recvd = rx.recv().await.unwrap();
      let msg: Specific = match recvd {
        ActorMsg::Msg(x) => x,
        ActorMsg::Serial(interface, bytes) => {
          <Specific as SpecificInterface<Unified>>::deserialize_as(
            interface, bytes,
          )
          .unwrap()
        }
      };
      actor.recv(&ctx, msg).await;
    }
  }
}
