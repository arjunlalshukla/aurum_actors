use async_trait::async_trait;
use aurum::cluster::crdt::CRDT;
use aurum::cluster::devices::{
  Charges, DeviceClient, DeviceClientCmd, DeviceClientConfig, DeviceServer, DeviceServerCmd,
  Devices, Manager,
};
use aurum::cluster::{Cluster, ClusterCmd, ClusterConfig, ClusterUpdate, HBRConfig};
use aurum::core::{Actor, ActorContext, ActorSignal, Host, LocalRef, Node, NodeConfig, Socket};
use aurum::testkit::{FailureConfigMap, LogLevel, LoggerMsg};
use aurum::{unify, AurumInterface};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;
use tokio::sync::mpsc::{channel, Sender};
use CoordinatorMsg::*;

unify!(DeviceTestTypes = CoordinatorMsg | ServerMsg | ClientMsg);

const HOST: Host = Host::IP(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));

struct ClusterData(u16, BTreeSet<u16>);
struct ServerData(u16, Charges);
struct ClientData(u16, Manager);

#[derive(Clone, Copy)]
enum ConvergenceType {
  Cluster,
  Devices,
}

struct TestServer {
  actor: LocalRef<ServerMsg>,
  charges: BTreeSet<u16>,
  node: Node<DeviceTestTypes>,
  view: Devices,
}

struct TestClient {
  actor: LocalRef<ClientMsg>,
  manager: Option<u16>,
  node: Node<DeviceTestTypes>,
}

#[derive(AurumInterface)]
#[aurum(local)]
enum CoordinatorMsg {
  Cluster(ClusterData),
  Server(ServerData),
  Client(ClientData),
  KillServer(u16),
  KillClient(u16),
  SpawnServer(u16, Vec<u16>),
  SpawnClient(u16, Vec<u16>),
  WaitForConvergence(ConvergenceType),
  Done,
}

struct Coordinator {
  clr_cfg: ClusterConfig,
  hbr_cfg: HBRConfig,
  fail_map: FailureConfigMap,
  cli_cfg: DeviceClientConfig,

  servers: BTreeMap<u16, TestServer>,
  clients: BTreeMap<u16, TestClient>,

  convergence: BTreeSet<u16>,
  converged: BTreeSet<u16>,

  queue: Vec<CoordinatorMsg>,
  waiting: Option<ConvergenceType>,
  notification: Sender<()>,
}
impl Coordinator {
  fn device_convergence_reached(&self) -> bool {
    let mut clients = BTreeSet::new();
    for (port, svr) in &self.servers {
      for c in svr.charges.iter() {
        let exclusive = clients.insert(*c);
        let is_manager = Some(*port) == self.clients.get(c).map(|t| t.manager).flatten();
        if !exclusive || !is_manager {
          return false;
        }
      }
    }
    let clients_valid = self.clients.iter().all(|(c, test)| {
      clients.contains(c) && test.manager.filter(|p| self.servers.contains_key(p)).is_some()
    });
    let crdt_converged = self
      .servers
      .values()
      .next()
      .map(|first| self.servers.values().skip(1).all(|other| other.view == first.view))
      .unwrap_or(true);
    crdt_converged && clients_valid
  }

  fn cluster_convergence_reached(&self) -> bool {
    self.servers.keys().all(|k| self.converged.contains(k))
  }

  fn check_convergence(&mut self, ctx: &ActorContext<DeviceTestTypes, CoordinatorMsg>) {
    if let Some(mode) = self.waiting {
      let converged = match mode {
        ConvergenceType::Cluster => {
          if self.cluster_convergence_reached() {
            println!("CLUSTER CONVERGENCE reached!");
            true
          } else {
            false
          }
        }
        ConvergenceType::Devices => {
          if self.device_convergence_reached() {
            let mut s = String::new();
            writeln!(s, "DEVICE CONVERGENCE reached!").unwrap();
            for (port, svr) in &self.servers {
              writeln!(s, "{} CHARGES - {:?}", port, svr.charges).unwrap();
            }
            for (port, client) in &self.clients {
              writeln!(s, "{} MANAGER - {:?}", port, client.manager).unwrap();
            }
            writeln!(s, "---").unwrap();
            println!("{}", s);
            true
          } else {
            false
          }
        }
      };
      if converged {
        self.waiting = None;
        let my_ref = ctx.local_interface();
        for msg in self.queue.drain(..) {
          my_ref.send(msg);
        }
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
      Cluster(ClusterData(port, view)) => {
        if view == self.convergence {
          self.converged.insert(port);
        } else {
          self.converged.remove(&port);
        }
        self.check_convergence(ctx);
      }
      Server(ServerData(port, data)) => {
        let charges = data.0.iter().map(|x| x.socket.udp).collect();
        println!("{} CHARGES - {:?}", port, charges);
        let svr = self.servers.get_mut(&port).unwrap();
        svr.charges = charges;
        svr.view = data.1;
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
        if self.waiting.is_some() {
          self.queue.push(KillServer(port));
          return;
        }
        println!("KILLING server on port {}", port);
        let node = self.servers.remove(&port).unwrap();
        node.node.log(LoggerMsg::SetLevel(LogLevel::Off));
        node.actor.signal(ActorSignal::Term);
        self.convergence.remove(&port);
        self.converged.clear();
      }
      KillClient(port) => {
        if self.waiting.is_some() {
          self.queue.push(KillClient(port));
          return;
        }
        println!("KILLING client on port {}", port);
        let node = self.clients.remove(&port).unwrap();
        node.node.log(LoggerMsg::SetLevel(LogLevel::Off));
        node.actor.signal(ActorSignal::Term);
      }
      SpawnClient(port, seeds) => {
        if self.waiting.is_some() {
          self.queue.push(SpawnClient(port, seeds));
          return;
        }
        let socket = Socket::new(HOST.clone(), port, 0);
        let mut config = NodeConfig::default();
        config.socket = socket.clone();
        let node = Node::<DeviceTestTypes>::new(config).await.unwrap();
        let mut cli_cfg = self.cli_cfg.clone();
        cli_cfg.seeds = seeds.into_iter().map(|p| Socket::new(HOST.clone(), p, 0)).collect();
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
        let recvr = node.spawn(false, recvr, "".to_string(), false).local().clone().unwrap();
        let entry = TestClient {
          actor: recvr,
          manager: None,
          node: node,
        };
        self.clients.insert(port, entry);
      }
      SpawnServer(port, seeds) => {
        if self.waiting.is_some() {
          self.queue.push(SpawnServer(port, seeds));
          return;
        }
        let socket = Socket::new(HOST.clone(), port, 0);
        let mut config = NodeConfig::default();
        config.socket = socket.clone();
        let node = Node::<DeviceTestTypes>::new(config).await.unwrap();
        let mut clr_cfg = self.clr_cfg.clone();
        clr_cfg.seed_nodes = seeds.iter().map(|p| Socket::new(HOST.clone(), *p, 0)).collect();
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
        let recvr = node.spawn(false, recvr, "".to_string(), false).local().clone().unwrap();

        let entry = TestServer {
          actor: recvr,
          charges: BTreeSet::new(),
          node: node,
          view: Devices::minimum(),
        };
        self.servers.insert(port, entry);
        self.convergence.insert(port);
        self.converged.clear();
      }
      WaitForConvergence(mode) => {
        if self.waiting.is_some() {
          self.queue.push(WaitForConvergence(mode));
          return;
        }
        match mode {
          ConvergenceType::Cluster => {
            if !self.cluster_convergence_reached() {
              println!("Waiting for CLUSTER CONVERGENCE");
              self.waiting = Some(ConvergenceType::Cluster);
              self.queue.clear();
            } else {
              println!("CLUSTER CONVERGENCE already reached");
            }
          }
          ConvergenceType::Devices => {
            if !self.device_convergence_reached() {
              println!("Waiting for DEVICES CONVERGENCE");
              self.waiting = Some(ConvergenceType::Devices);
              self.queue.clear();
            } else {
              println!("DEVICES CONVERGENCE already reached");
            }
          }
        }
      }
      Done => {
        if self.waiting.is_some() {
          self.queue.push(Done);
        } else {
          println!("Done!");
          self.notification.send(()).await.unwrap();
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
  supervisor: LocalRef<CoordinatorMsg>,
  client: LocalRef<DeviceClientCmd>,
}
#[async_trait]
impl Actor<DeviceTestTypes, ClientMsg> for Client {
  async fn pre_start(&mut self, ctx: &ActorContext<DeviceTestTypes, ClientMsg>) {
    let msg = DeviceClientCmd::Subscribe(ctx.local_interface());
    self.client.send(msg);
  }

  async fn recv(&mut self, ctx: &ActorContext<DeviceTestTypes, ClientMsg>, msg: ClientMsg) {
    match msg {
      ClientMsg::Manager(manager) => {
        let msg = Client(ClientData(ctx.node.socket().udp, manager));
        self.supervisor.send(msg);
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
  #[aurum(local)]
  Update(ClusterUpdate),
}

struct Server {
  supervisor: LocalRef<CoordinatorMsg>,
  cluster: LocalRef<ClusterCmd>,
  devices: LocalRef<DeviceServerCmd>,
}
#[async_trait]
impl Actor<DeviceTestTypes, ServerMsg> for Server {
  async fn pre_start(&mut self, ctx: &ActorContext<DeviceTestTypes, ServerMsg>) {
    let msg = DeviceServerCmd::Subscribe(ctx.local_interface());
    self.devices.send(msg);
    let msg = ClusterCmd::Subscribe(ctx.local_interface());
    self.cluster.send(msg);
  }

  async fn recv(&mut self, ctx: &ActorContext<DeviceTestTypes, ServerMsg>, msg: ServerMsg) {
    match msg {
      ServerMsg::Devices(charges) => {
        let msg = Server(ServerData(ctx.node.socket().udp, charges));
        self.supervisor.send(msg);
      }
      ServerMsg::Update(update) => {
        let view = update.nodes.iter().map(|m| m.socket.udp).collect();
        let msg = Cluster(ClusterData(ctx.node.socket().udp, view));
        self.supervisor.send(msg);
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
  timeout: Duration,
) {
  let socket = Socket::new(Host::DNS("127.0.0.1".to_string()), 5500, 0);
  let mut config = NodeConfig::default();
  config.socket = socket.clone();
  let node = Node::<DeviceTestTypes>::new_sync(config).unwrap();
  let (tx, mut rx) = channel(1);
  let actor = Coordinator {
    clr_cfg: clr_cfg,
    hbr_cfg: hbr_cfg,
    fail_map: fail_map,
    cli_cfg: cli_cfg,
    servers: BTreeMap::new(),
    clients: BTreeMap::new(),
    convergence: BTreeSet::new(),
    converged: BTreeSet::new(),
    queue: Vec::new(),
    waiting: None,
    notification: tx,
  };
  let coor = node.spawn(false, actor, "".to_string(), false).local().clone().unwrap();
  for e in events {
    coor.send(e);
  }
  node.rt().block_on(async { tokio::time::timeout(timeout, rx.recv()).await.unwrap().unwrap() });
}

#[test]
fn devices_test_perfect() {
  let events = vec![
    SpawnServer(3001, vec![]),
    SpawnServer(3002, vec![3001]),
    SpawnServer(3003, vec![3001]),
    WaitForConvergence(ConvergenceType::Cluster),
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
    WaitForConvergence(ConvergenceType::Devices),
    KillClient(4003),
    KillClient(4004),
    WaitForConvergence(ConvergenceType::Devices),
    KillServer(3002),
    WaitForConvergence(ConvergenceType::Cluster),
    WaitForConvergence(ConvergenceType::Devices),
    Done,
  ];
  let mut fail_map = FailureConfigMap::default();
  fail_map.cluster_wide.drop_prob = 0.25;
  fail_map.cluster_wide.delay = Some((Duration::from_millis(20), Duration::from_millis(50)));
  let mut clr_cfg = ClusterConfig::default();
  clr_cfg.vnodes = 100;
  clr_cfg.num_pings = 20;
  clr_cfg.ping_timeout = Duration::from_millis(200);
  let mut hbr_cfg = HBRConfig::default();
  hbr_cfg.req_tries = 1;
  hbr_cfg.req_timeout = Duration::from_millis(200);
  let mut cli_cfg = DeviceClientConfig::default();
  cli_cfg.initial_interval = Duration::from_millis(300);
  let timeout = Duration::from_millis(15_000);
  run_cluster_test(events, fail_map, clr_cfg, hbr_cfg, cli_cfg, timeout);
}
