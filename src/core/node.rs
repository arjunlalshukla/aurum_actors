use crate::core::{
  run_secondary, run_single, udp_receiver, Actor, ActorContext, ActorMsg, Case,
  LocalRef, Registry, RegistryMsg, Socket, SpecificInterface, UnifiedBounds,
};
use std::sync::Arc;
use tokio::runtime::{Builder, Runtime};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

pub(crate) struct NodeImpl<Unified: UnifiedBounds> {
  pub(crate) socket: Socket,
  pub(crate) registry: LocalRef<RegistryMsg<Unified>>,
  pub(crate) rt: Runtime,
}

#[derive(Clone)]
pub struct Node<Unified: UnifiedBounds> {
  pub(crate) node: Arc<NodeImpl<Unified>>,
}
impl<Unified: UnifiedBounds + Case<RegistryMsg<Unified>>> Node<Unified> {
  pub fn new(socket: Socket, actor_threads: usize) -> Self {
    let rt = Builder::new_multi_thread()
      .enable_io()
      .worker_threads(actor_threads)
      .thread_name("tokio-thread")
      .thread_stack_size(3 * 1024 * 1024)
      .build()
      .unwrap();
    let (udp_tx, udp_rx) = tokio::sync::oneshot::channel();
    rt.spawn(udp_receiver::<Unified>(udp_rx));
    let (reg, reg_node_tx) =
      Self::start_codependent(&rt, Registry::new(), "registry".to_string());
    let node = Node {
      node: Arc::new(NodeImpl {
        socket: socket,
        registry: reg,
        rt: rt,
      }),
    };
    if reg_node_tx.send(node.clone()).is_err() {
      panic!("Could not send node to registry");
    }
    if udp_tx.send(node.clone()).is_err() {
      panic!("Could not send node to udp receiver");
    }
    println!("Started node");
    node
  }

  pub fn socket(&self) -> &Socket {
    &self.node.socket
  }

  pub fn registry(&self, msg: RegistryMsg<Unified>) -> bool {
    self.node.registry.send(msg)
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
      run_single(node_rx.recv().await.unwrap(), actor, name, tx, rx, false)
        .await
    });
    ret
  }

  pub fn spawn<Specific, A>(
    &self,
    double: bool,
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
    if double {
      self
        .node
        .rt
        .spawn(run_secondary(node, actor, name, tx, rx, register));
    } else {
      self
        .node
        .rt
        .spawn(run_single(node, actor, name, tx, rx, register));
    }
    ret
  }
}
