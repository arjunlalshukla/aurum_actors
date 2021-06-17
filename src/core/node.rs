use crate::core::{
  run_single_timeout, udp_receiver, unit_secondary, unit_single, Actor, ActorContext, ActorMsg,
  ActorName, ActorRef, ActorSignal, Case, Destination, Interpretations, LocalRef, MessagePackets,
  Registry, RegistryMsg, Socket, SpecificInterface, TimeoutActor, UnifiedType,
};
use crate::testkit::{FailureConfigMap, FailureMode, LogLevel, Logger, LoggerMsg};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use serde::{de::DeserializeOwned, Serialize};
use std::io::{Error, ErrorKind};
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::runtime::{Builder, Runtime};
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::oneshot::{channel, Sender};
use tokio::task::JoinHandle;

pub struct NodeConfig {
  pub socket: Socket,
  pub actor_threads: usize,
  pub actor_thread_stack_size: usize,
  pub compute_threads: usize,
}
impl Default for NodeConfig {
  fn default() -> Self {
    Self {
      socket: Socket::default(),
      actor_threads: num_cpus::get(),
      actor_thread_stack_size: 3 * 1024 * 1024,
      compute_threads: num_cpus::get(),
    }
  }
}

struct NodeImpl<U: UnifiedType> {
  socket: Socket,
  udp: UdpSocket,
  registry: LocalRef<RegistryMsg<U>>,
  logger: LocalRef<LoggerMsg>,
  rt: Runtime,
}

#[derive(Clone)]
pub struct Node<U: UnifiedType> {
  node: Arc<NodeImpl<U>>,
}
impl<U: UnifiedType> Node<U> {
  pub fn new_sync(config: NodeConfig) -> std::io::Result<Self> {
    let rt = Builder::new_multi_thread()
      .enable_io()
      .enable_time()
      .worker_threads(config.actor_threads)
      .thread_name("tokio-thread")
      .thread_stack_size(config.actor_thread_stack_size)
      .build()?;
    let udp = rt.block_on(UdpSocket::bind((Ipv4Addr::UNSPECIFIED, config.socket.udp)))?;
    Self::new_priv(rt, udp, config)
  }

  pub async fn new(config: NodeConfig) -> std::io::Result<Self> {
    let rt = Builder::new_multi_thread()
      .enable_io()
      .enable_time()
      .worker_threads(config.actor_threads)
      .thread_name("tokio-thread")
      .thread_stack_size(config.actor_thread_stack_size)
      .build()?;
    let udp = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, config.socket.udp)).await?;
    Self::new_priv(rt, udp, config)
  }

  fn new_priv(rt: Runtime, udp: UdpSocket, config: NodeConfig) -> std::io::Result<Self> {
    let (reg, reg_node_tx) = Self::start_codependent(&rt, Registry::new(), "registry".to_string());
    let (log, log_node_tx) =
      Self::start_codependent(&rt, Logger::new(LogLevel::Trace), "node_logger".to_string());
    let node = Node {
      node: Arc::new(NodeImpl {
        socket: config.socket,
        udp: udp,
        registry: reg,
        logger: log,
        rt: rt,
      }),
    };
    reg_node_tx.send(node.clone()).map_err(|_| Error::new(ErrorKind::NotFound, "Registry"))?;
    log_node_tx.send(node.clone()).map_err(|_| Error::new(ErrorKind::NotFound, "Logger"))?;
    node.node.rt.spawn(udp_receiver::<U>(node.clone()));
    Ok(node)
  }

  pub(in crate::core) fn udp_socket(&self) -> &UdpSocket {
    &self.node.udp
  }

  pub fn socket(&self) -> &Socket {
    &self.node.socket
  }

  pub fn registry(&self, msg: RegistryMsg<U>) -> bool {
    self.node.registry.send(msg)
  }

  pub fn log(&self, msg: LoggerMsg) -> bool {
    self.node.logger.send(msg)
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

  fn start_codependent<S, A>(rt: &Runtime, actor: A, name: String) -> (LocalRef<S>, Sender<Self>)
  where
    A: Actor<U, S> + Send + 'static,
    S: SpecificInterface<U>,
    U: Case<S>,
  {
    let (tx, rx) = unbounded_channel::<ActorMsg<U, S>>();
    let (node_tx, node_rx) = channel::<Node<U>>();
    let ret = (ActorContext::<U, S>::create_local(tx.clone()), node_tx);
    rt.spawn(async move {
      let ctx = ActorContext {
        tx: tx,
        name: ActorName::new::<S>(name),
        node: node_rx.await.unwrap(),
      };
      unit_single(actor, ctx, rx, false).await
    });
    ret
  }

  pub fn spawn<S, A>(&self, double: bool, actor: A, name: String, register: bool) -> ActorRef<U, S>
  where
    U: Case<S>,
    S: SpecificInterface<U>,
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
    S: SpecificInterface<U>,
    A: TimeoutActor<U, S> + Send + 'static,
  {
    let (tx, rx) = unbounded_channel::<ActorMsg<U, S>>();
    let ctx = ActorContext {
      tx: tx,
      name: ActorName::new::<S>(name),
      node: self.clone(),
    };
    let ret = ctx.interface();
    self.node.rt.spawn(run_single_timeout(actor, ctx, rx, register, timeout));
    ret
  }

  pub async fn udp_msg<I>(&self, socket: &Socket, dest: &Destination<U, I>, msg: &I)
  where
    I: Serialize + DeserializeOwned,
    U: Case<I>,
  {
    self.udp_send(&socket, &dest, Interpretations::Message, msg).await;
  }

  pub async fn udp_signal<I>(&self, socket: &Socket, dest: &Destination<U, I>, sig: &ActorSignal)
  where
    U: Case<I>,
  {
    self.udp_send(&socket, &dest, Interpretations::Signal, sig).await;
  }

  async fn udp_send<I, T>(
    &self,
    socket: &Socket,
    dest: &Destination<U, I>,
    intp: Interpretations,
    msg: &T,
  ) where
    T: Serialize + DeserializeOwned,
    U: Case<I>,
  {
    let addrs = socket.as_udp_addr().await.unwrap();
    let addr = addrs.iter().next().expect(format!("No resolution for {:?}", socket).as_str());
    MessagePackets::new(msg, intp, dest).move_to(&self.node.udp, addr).await;
  }

  pub async fn udp_msg_unreliable_msg<I>(
    &self,
    socket: &Socket,
    dest: &Destination<U, I>,
    msg: &I,
    fail_cfg: &FailureConfigMap,
  ) where
    I: Serialize + DeserializeOwned,
    U: Case<I>,
  {
    self.udp_unreliable_msg(socket, dest, Interpretations::Message, msg, fail_cfg).await;
  }

  pub async fn udp_signal_unreliable_msg<I>(
    &self,
    socket: &Socket,
    dest: &Destination<U, I>,
    sig: &ActorSignal,
    fail_cfg: &FailureConfigMap,
  ) where
    U: Case<I>,
  {
    self.udp_unreliable_msg(socket, dest, Interpretations::Signal, sig, fail_cfg).await;
  }

  async fn udp_unreliable_msg<I, T>(
    &self,
    socket: &Socket,
    dest: &Destination<U, I>,
    intp: Interpretations,
    msg: &T,
    fail_map: &FailureConfigMap,
  ) where
    T: Serialize + DeserializeOwned,
    U: Case<I>,
  {
    let fail_cfg = fail_map.get(socket);
    let addrs = socket.as_udp_addr().await.unwrap();
    let addr = addrs.into_iter().next().expect(format!("No resolution for {:?}", socket).as_str());
    let packets = MessagePackets::new(msg, intp, dest);
    let dur = fail_cfg.delay.map(|(min, max)| {
      let range = min.as_millis()..=max.as_millis();
      Duration::from_millis(SmallRng::from_entropy().gen_range(range) as u64)
    });
    // We need to to the serialization work, even if the send fails.
    if rand::random::<f64>() >= fail_cfg.drop_prob {
      if let Some(dur) = dur {
        let node = self.clone();
        self.rt().spawn(async move {
          tokio::time::sleep(dur).await;
          packets.move_to(node.udp_socket(), &addr).await;
        });
      } else {
        packets.move_to(self.udp_socket(), &addr).await;
      }
    }
  }

  pub async fn udp_select<I>(
    &self,
    socket: &Socket,
    dest: &Destination<U, I>,
    msg: &I,
    mode: FailureMode,
    fail_map: &FailureConfigMap,
  ) where
    I: Serialize + DeserializeOwned,
    U: Case<I>,
  {
    match mode {
      FailureMode::None => {
        self.udp_msg(socket, dest, msg).await;
      }
      FailureMode::Message => {
        self.udp_msg_unreliable_msg(socket, dest, msg, fail_map).await;
      }
      FailureMode::Packet => {
        todo!()
      }
    }
  }
}
