use crate::core::{
  run_single_timeout, udp_receiver, unit_secondary, unit_single, Actor,
  ActorContext, ActorMsg, ActorName, ActorRef, Case, LocalRef, Registry,
  RegistryMsg, Socket, SpecificInterface, TimeoutActor, UnifiedBounds,
};
use crate::testkit::{Logger, LoggerMsg, LogLevel};
use std::io::{Error, ErrorKind};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::{Builder, Runtime};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio::task::JoinHandle;

struct NodeImpl<U: UnifiedBounds> {
  socket: Socket,
  registry: LocalRef<RegistryMsg<U>>,
  logger: LocalRef<LoggerMsg>,
  rt: Runtime,
}

#[derive(Clone)]
pub struct Node<U: UnifiedBounds> {
  node: Arc<NodeImpl<U>>,
}
impl<U: UnifiedBounds> Node<U> {
  pub fn new(socket: Socket, actor_threads: usize) -> std::io::Result<Self> {
    let rt = Builder::new_multi_thread()
      .enable_io()
      .enable_time()
      .worker_threads(actor_threads)
      .thread_name("tokio-thread")
      .thread_stack_size(3 * 1024 * 1024)
      .build()?;
    let (reg, reg_node_tx) =
      Self::start_codependent(&rt, Registry::new(), "registry".to_string());
    //let (log, log_node_tx) = Self::start_codependent(&rt, Logger::new(LogLevel::Trace), "registry".to_string());
    let node = Node {
      node: Arc::new(NodeImpl {
        socket: socket,
        registry: reg,
        logger: LocalRef::void(),
        rt: rt,
      }),
    };
    reg_node_tx
      .send(node.clone())
      .map_err(|_| Error::new(ErrorKind::NotFound, "Registry"))?;
    node.node.rt.spawn(udp_receiver::<U>(node.clone()));
    
    Ok(node)
  }

  pub fn socket(&self) -> &Socket {
    &self.node.socket
  }

  pub fn registry(&self, msg: RegistryMsg<U>) -> bool {
    self.node.registry.send(msg)
  }

  pub fn rt(&self) -> &Runtime {
    &self.node.rt
  }

  pub fn schedule<F>(&self, delay: Duration, op: F) -> JoinHandle<()>
  where
    F: 'static + Send + FnOnce() -> (),
  {
    self.node.rt.spawn(async move {
      tokio::time::sleep(delay).await;
      op();
    })
  }

  pub fn schedule_local_msg<T: Send + 'static>(
    &self,
    delay: Duration,
    actor: LocalRef<T>,
    msg: T,
  ) -> JoinHandle<bool> {
    self.node.rt.spawn(async move {
      tokio::time::sleep(delay).await;
      actor.send(msg)
    })
  }

  fn start_codependent<S, A>(
    rt: &Runtime,
    actor: A,
    name: String,
  ) -> (LocalRef<S>, UnboundedSender<Self>)
  where
    A: Actor<U, S> + Send + 'static,
    S: 'static + Send + SpecificInterface<U>,
    U: Case<S>,
  {
    let (tx, rx) = unbounded_channel::<ActorMsg<U, S>>();
    let (node_tx, mut node_rx) = unbounded_channel::<Node<U>>();
    let ret = (ActorContext::<U, S>::create_local(tx.clone()), node_tx);
    rt.spawn(async move {
      let ctx = ActorContext {
        tx: tx,
        name: ActorName::new::<S>(name),
        node: node_rx.recv().await.unwrap(),
      };
      unit_single(actor, ctx, rx, false).await
    });
    ret
  }

  pub fn spawn<S, A>(
    &self,
    double: bool,
    actor: A,
    name: String,
    register: bool,
  ) -> ActorRef<U, S>
  where
    U: Case<S>,
    S: 'static + Send + SpecificInterface<U>,
    A: Actor<U, S> + Send + 'static,
  {
    let (tx, rx) = unbounded_channel::<ActorMsg<U, S>>();
    let ctx = ActorContext {
      tx: tx,
      name: ActorName::new::<S>(name),
      node: self.clone(),
    };
    let ret = ctx.interface();
    if double {
      self.node.rt.spawn(unit_secondary(actor, ctx, rx, register));
    } else {
      self.node.rt.spawn(unit_single(actor, ctx, rx, register));
    }
    ret
  }

  pub fn spawn_timeout<S, A>(
    &self,
    actor: A,
    name: String,
    register: bool,
    timeout: Duration,
  ) -> ActorRef<U, S>
  where
    U: Case<S>,
    S: 'static + Send + SpecificInterface<U>,
    A: TimeoutActor<U, S> + Send + 'static,
  {
    let (tx, rx) = unbounded_channel::<ActorMsg<U, S>>();
    let ctx = ActorContext {
      tx: tx,
      name: ActorName::new::<S>(name),
      node: self.clone(),
    };
    let ret = ctx.interface();
    self
      .node
      .rt
      .spawn(run_single_timeout(actor, ctx, rx, register, timeout));
    ret
  }
}
