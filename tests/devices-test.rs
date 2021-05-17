use async_trait::async_trait;
use aurum::cluster::devices::{
  Charges, DeviceClient, DeviceClientCmd, DeviceClientConfig, DeviceServer,
  DeviceServerCmd, Manager,
};
use aurum::cluster::{Cluster, ClusterCmd, ClusterConfig, HBRConfig};
use aurum::core::{
  Actor, ActorContext, ActorSignal, Host, LocalRef, Node, Socket,
};
use aurum::testkit::{FailureConfigMap, LogLevel, LoggerMsg};
use aurum::{unify, AurumInterface};
use crossbeam::channel::{unbounded, Sender};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;
use CoordinatorMsg::*;

unify!(DeviceTestTypes = CoordinatorMsg | ServerMsg | ClientMsg);

const HOST: Host = Host::IP(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));

struct ServerData(u16, Charges);
struct ClientData(u16, Manager);

struct TestServer {
  actor: LocalRef<ServerMsg>,
  charges: BTreeSet<u16>,
  node: Node<DeviceTestTypes>,
}

struct TestClient {
  actor: LocalRef<ClientMsg>,
  manager: Option<u16>,
  node: Node<DeviceTestTypes>,
}

#[derive(AurumInterface)]
#[aurum(local)]
enum CoordinatorMsg {
  #[aurum(local)]
  Server(ServerData),
  #[aurum(local)]
  Client(ClientData),
  KillServer(u16),
  KillClient(u16),
  SpawnServer(u16, Vec<u16>),
  SpawnClient(u16, Vec<u16>),
  Wait(Duration),
  WaitForConvergence,
  Done,
}

struct Coordinator {
  clr_cfg: ClusterConfig,
  hbr_cfg: HBRConfig,
  fail_map: FailureConfigMap,
  cli_cfg: DeviceClientConfig,

  servers: BTreeMap<u16, TestServer>,
  clients: BTreeMap<u16, TestClient>,

  queue: Vec<CoordinatorMsg>,
  waiting: bool,
  notification: Sender<()>,
}
impl Coordinator {
  fn convergence_reached(&self) -> bool {
    let mut clients = BTreeSet::new();
    for (port, svr) in &self.servers {
      for c in svr.charges.iter() {
        let exclusive = clients.insert(*c);
        let is_manager =
          Some(*port) == self.clients.get(c).map(|t| t.manager).flatten();
        if !exclusive || !is_manager {
          return false;
        }
      }
    }
    self
      .clients
      .iter()
      .all(|(c, test)| clients.contains(c) && test.manager.is_some())
  }

  fn check_convergence(
    &mut self,
    ctx: &ActorContext<DeviceTestTypes, CoordinatorMsg>,
  ) {
    if self.waiting && self.convergence_reached() {
      let mut s = String::new();
      writeln!(s, "CONVERGENCE reached!").unwrap();
      for (port, svr) in &self.servers {
        writeln!(s, "{} CHARGES - {:?}", port, svr.charges).unwrap();
      }
      for (port, client) in &self.clients {
        writeln!(s, "{} MANAGER - {:?}", port, client.manager).unwrap();
      }
      writeln!(s, "---").unwrap();
      println!("{}", s);
      self.waiting = false;
      let my_ref = ctx.local_interface();
      for msg in self.queue.drain(..) {
        my_ref.send(msg);
      }
    }
  }
}
#[async_trait]
impl Actor<DeviceTestTypes, CoordinatorMsg> for Coordinator {
  async fn recv(
    &mut self,
    ctx: &ActorContext<DeviceTestTypes, CoordinatorMsg>,
    msg: CoordinatorMsg,
  ) {
    match msg {
      Server(ServerData(port, charges)) => {
        let charges = charges.0.iter().map(|x| x.socket.udp).collect();
        println!("{} CHARGES - {:?}", port, charges);
        let svr = self.servers.get_mut(&port).unwrap();
        svr.charges = charges;
        self.check_convergence(ctx);
      }
      Client(ClientData(port, manager)) => {
        let manager = manager.0.map(|x| x.udp);
        println!("{} MANAGER - {:?}", port, manager);
        let client = self.clients.get_mut(&port).unwrap();
        client.manager = manager;
        self.check_convergence(ctx);
      }
      KillServer(port) => {
        if self.waiting {
          self.queue.push(KillServer(port));
          return;
        }
        println!("KILLING server on port {}", port);
        let node = self.servers.remove(&port).unwrap();
        node.node.log(LoggerMsg::SetLevel(LogLevel::Off));
        node.actor.signal(ActorSignal::Term);
      }
      KillClient(port) => {
        if self.waiting {
          self.queue.push(KillClient(port));
          return;
        }
        println!("KILLING client on port {}", port);
        let node = self.clients.remove(&port).unwrap();
        node.node.log(LoggerMsg::SetLevel(LogLevel::Off));
        node.actor.signal(ActorSignal::Term);
      }
      SpawnClient(port, seeds) => {
        if self.waiting {
          self.queue.push(SpawnServer(port, seeds));
          return;
        }
        let socket = Socket::new(HOST.clone(), port, 0);
        let node = Node::<DeviceTestTypes>::new(socket.clone(), 1).unwrap();
        let mut cli_cfg = self.cli_cfg.clone();
        cli_cfg.seeds = seeds
          .into_iter()
          .map(|p| Socket::new(HOST.clone(), p, 0))
          .collect();
        let client = DeviceClient::new(
          &node,
          cli_cfg,
          "test-devices".to_string(),
          self.fail_map.clone(),
          vec![],
        );
        let recvr = Client {
          supervisor: ctx.local_interface(),
          client: client,
        };
        let recvr = node
          .spawn(false, recvr, "".to_string(), false)
          .local()
          .clone()
          .unwrap();
        let entry = TestClient {
          actor: recvr,
          manager: None,
          node: node,
        };
        self.clients.insert(port, entry);
      }
      SpawnServer(port, seeds) => {
        if self.waiting {
          self.queue.push(SpawnServer(port, seeds));
          return;
        }
        let socket = Socket::new(HOST.clone(), port, 0);
        let node = Node::<DeviceTestTypes>::new(socket.clone(), 1).unwrap();
        let mut clr_cfg = self.clr_cfg.clone();
        clr_cfg.seed_nodes = seeds
          .iter()
          .map(|p| Socket::new(HOST.clone(), *p, 0))
          .collect();
        let cluster = Cluster::new(
          &node,
          "test-devices".to_string(),
          vec![],
          self.fail_map.clone(),
          clr_cfg,
          self.hbr_cfg.clone(),
        );
        let devices = DeviceServer::new(
          &node,
          cluster.clone(),
          vec![],
          "test-devices".to_string(),
          self.fail_map.clone(),
        );
        let recvr = Server {
          supervisor: ctx.local_interface(),
          cluster: cluster,
          devices: devices,
        };
        let recvr = node
          .spawn(false, recvr, "".to_string(), false)
          .local()
          .clone()
          .unwrap();
        let entry = TestServer {
          actor: recvr,
          charges: BTreeSet::new(),
          node: node,
        };
        self.servers.insert(port, entry);
      }
      Wait(dur) => tokio::time::sleep(dur).await,
      WaitForConvergence => {
        if self.waiting {
          self.queue.push(WaitForConvergence);
          return;
        }
        if !self.convergence_reached() {
          println!("Waiting for CONVERGENCE");
          self.waiting = true;
          self.queue.clear();
        } else {
          println!("CONVERGENCE already reached");
        }
      }
      Done => {
        if self.waiting {
          self.queue.push(Done);
          return;
        }
        if self.convergence_reached() {
          println!("Done!");
          self.notification.send(()).unwrap();
        } else {
          println!("Waiting for CONVERGENCE");
          self.waiting = true;
          self.queue.clear();
          self.queue.push(Done);
        }
      }
    }
  }
}

#[derive(AurumInterface)]
#[aurum(local)]
enum ClientMsg {
  #[aurum(local)]
  Manager(Manager),
}

struct Client {
  supervisor: LocalRef<ClientData>,
  client: LocalRef<DeviceClientCmd>,
}
#[async_trait]
impl Actor<DeviceTestTypes, ClientMsg> for Client {
  async fn pre_start(
    &mut self,
    ctx: &ActorContext<DeviceTestTypes, ClientMsg>,
  ) {
    self
      .client
      .send(DeviceClientCmd::Subscribe(ctx.local_interface()));
  }

  async fn recv(
    &mut self,
    ctx: &ActorContext<DeviceTestTypes, ClientMsg>,
    msg: ClientMsg,
  ) {
    match msg {
      ClientMsg::Manager(manager) => {
        self
          .supervisor
          .send(ClientData(ctx.node.socket().udp, manager));
      }
    }
  }

  async fn post_stop(&mut self, _: &ActorContext<DeviceTestTypes, ClientMsg>) {
    self.client.signal(ActorSignal::Term);
  }
}

#[derive(AurumInterface)]
#[aurum(local)]
enum ServerMsg {
  #[aurum(local)]
  Devices(Charges),
}

struct Server {
  supervisor: LocalRef<ServerData>,
  cluster: LocalRef<ClusterCmd>,
  devices: LocalRef<DeviceServerCmd>,
}
#[async_trait]
impl Actor<DeviceTestTypes, ServerMsg> for Server {
  async fn pre_start(
    &mut self,
    ctx: &ActorContext<DeviceTestTypes, ServerMsg>,
  ) {
    self
      .devices
      .send(DeviceServerCmd::Subscribe(ctx.local_interface()));
  }

  async fn recv(
    &mut self,
    ctx: &ActorContext<DeviceTestTypes, ServerMsg>,
    msg: ServerMsg,
  ) {
    match msg {
      ServerMsg::Devices(charges) => {
        self
          .supervisor
          .send(ServerData(ctx.node.socket().udp, charges));
      }
    }
  }

  async fn post_stop(&mut self, _: &ActorContext<DeviceTestTypes, ServerMsg>) {
    self.cluster.signal(ActorSignal::Term);
    self.devices.signal(ActorSignal::Term);
  }
}

fn run_cluster_test(
  events: Vec<CoordinatorMsg>,
  fail_map: FailureConfigMap,
  clr_cfg: ClusterConfig,
  hbr_cfg: HBRConfig,
  cli_cfg: DeviceClientConfig,
) {
  let socket = Socket::new(Host::DNS("127.0.0.1".to_string()), 5500, 0);
  let node = Node::<DeviceTestTypes>::new(socket.clone(), 1).unwrap();
  let (tx, rx) = unbounded();
  let actor = Coordinator {
    clr_cfg: clr_cfg,
    hbr_cfg: hbr_cfg,
    fail_map: fail_map,
    cli_cfg: cli_cfg,
    servers: BTreeMap::new(),
    clients: BTreeMap::new(),
    queue: Vec::new(),
    waiting: false,
    notification: tx,
  };
  let coor = node
    .spawn(false, actor, "".to_string(), false)
    .local()
    .clone()
    .unwrap();
  for e in events {
    coor.send(e);
  }
  rx.recv_timeout(Duration::from_millis(10_000)).unwrap();
}

#[test]
fn devices_test_perfect() {
  let events = vec![
    SpawnServer(3001, vec![]),
    Wait(Duration::from_millis(20)),
    SpawnServer(3002, vec![3001]),
    SpawnServer(3003, vec![3001]),
    Wait(Duration::from_millis(100)),
    SpawnClient(4001, vec![3001]),
    SpawnClient(4002, vec![3001]),
    SpawnClient(4003, vec![3001]),
    SpawnClient(4004, vec![3001]),
    SpawnClient(4005, vec![3001]),
    SpawnClient(4006, vec![3001]),
    SpawnClient(4007, vec![3001]),
    SpawnClient(4008, vec![3001]),
    SpawnClient(4009, vec![3001]),
    SpawnClient(4010, vec![3001]),
    SpawnClient(4011, vec![3001]),
    SpawnClient(4012, vec![3001]),

    WaitForConvergence,
    KillClient(4003),
    KillClient(4004),
    WaitForConvergence,
    Done,
  ];
  let fail_map = FailureConfigMap::default();
  let mut clr_cfg = ClusterConfig::default();
  clr_cfg.vnodes = 3;
  clr_cfg.num_pings = 20;
  clr_cfg.ping_timeout = Duration::from_millis(50);
  let mut hbr_cfg = HBRConfig::default();
  hbr_cfg.req_tries = 1;
  hbr_cfg.req_timeout = Duration::from_millis(50);
  let mut cli_cfg = DeviceClientConfig::default();
  cli_cfg.initial_interval = Duration::from_millis(20);
  run_cluster_test(events, fail_map, clr_cfg, hbr_cfg, cli_cfg);
}
