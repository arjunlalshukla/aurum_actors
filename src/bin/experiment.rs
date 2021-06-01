use async_trait::async_trait;
use aurum::core::{
  Actor, ActorContext, ActorRef, Destination, Host, LocalRef, Node, NodeConfig,
  Socket,
};
use aurum::{unify, AurumInterface};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{channel, Sender};

unify!(
  BenchmarkTypes = ReportReceiverMsg
    | IoTBusinessMsg
    | LocalReportReceiverMsg
    | LocalIoTBusinessMsg
);

const CLUSTER_NAME: &'static str = "my-cool-device-cluster";

fn main() {
  // Exclude the command
  let mut args = std::env::args().skip(1);
  args.next().unwrap();
  let host = args.next().unwrap();
  let port = args.next().unwrap().parse().unwrap();
  let mode = args.next().unwrap();
  let mut config = NodeConfig::default();
  config.socket = Socket::new(Host::from(host), port, 0);
  let node = Node::<BenchmarkTypes>::new_sync(config).unwrap();
  let (tx, mut rx) = channel(1);

  match mode.as_str() {
    "server" => server(tx, &node, &mut args),
    "client" => client(tx, &node, &mut args),
    "system" => system(tx, &node, &mut args),
    _ => panic!("invalid mode {}", mode),
  }

  node.rt().block_on(rx.recv());
}

fn system(
  notify: Sender<()>,
  node: &Node<BenchmarkTypes>,
  _: &mut impl Iterator<Item = String>,
) {
  let reporter = LocalIoTBusiness {
    notify: notify.clone(),
  };
  let reporter = node
    .spawn(false, reporter, "".to_string(), false)
    .local()
    .clone()
    .unwrap();
  let recvr = LocalReportReceiver {
    notify: notify,
    client: reporter,
    reqs_sent: 0,
    reqs_recvd: 0,
    total: 0,
    start: Instant::now(),
    prev_print: 0,
  };
  node.spawn(false, recvr, "".to_string(), false);
}

fn server(
  notify: Sender<()>,
  node: &Node<BenchmarkTypes>,
  args: &mut impl Iterator<Item = String>,
) {
  let interval = Duration::from_millis(args.next().unwrap().parse().unwrap());
  let name = CLUSTER_NAME.to_string();
  let host = args.next().unwrap();
  let port = args.next().unwrap().parse().unwrap();
  let socket = Socket::new(Host::from(host), port, 1001);
  let recvr = ReportReceiver {
    notify: notify,
    client: socket,
    dest: Destination::new::<IoTBusinessMsg>(CLUSTER_NAME.to_string()),
    req_timeout: interval,
    reqs_sent: 0,
    reqs_recvd: 0,
    total: 0,
    start: Instant::now(),
    prev_print: 0,
  };
  node.spawn(false, recvr, name, true);
}

fn client(
  notify: Sender<()>,
  node: &Node<BenchmarkTypes>,
  _: &mut impl Iterator<Item = String>,
) {
  let name = CLUSTER_NAME.to_string();
  let actor = IoTBusiness { notify: notify };
  node.spawn(false, actor, name, true);
}

#[derive(AurumInterface, Serialize, Deserialize)]
enum LocalReportReceiverMsg {
  Report(bool),
  Print,
}
struct LocalReportReceiver {
  notify: Sender<()>,
  client: LocalRef<LocalIoTBusinessMsg>,
  reqs_sent: u64,
  reqs_recvd: u64,
  total: u128,
  start: Instant,
  prev_print: u64,
}
#[async_trait]
impl Actor<BenchmarkTypes, LocalReportReceiverMsg> for LocalReportReceiver {
  async fn pre_start(
    &mut self,
    ctx: &ActorContext<BenchmarkTypes, LocalReportReceiverMsg>,
  ) {
    self.reqs_sent = 1;
    self.reqs_recvd = 0;
    ctx.node.schedule_local_msg(
      Duration::from_millis(1000),
      ctx.local_interface(),
      LocalReportReceiverMsg::Print,
    );
    self
      .client
      .send(LocalIoTBusinessMsg::ReportReq(ctx.local_interface()));
  }

  async fn recv(
    &mut self,
    ctx: &ActorContext<BenchmarkTypes, LocalReportReceiverMsg>,
    msg: LocalReportReceiverMsg,
  ) {
    match msg {
      LocalReportReceiverMsg::Report(contents) => {
        self.reqs_recvd += 1;
        self.total += contents as u128;
        self.reqs_sent += 1;
        self
          .client
          .send(LocalIoTBusinessMsg::ReportReq(ctx.local_interface()));
      }
      LocalReportReceiverMsg::Print => {
        let e = self.start.elapsed().as_secs_f64();
        println!(
          "Recvs: {}; Elapsed: {}; Rate; {}; Since Last: {}; Total: {}",
          self.reqs_recvd,
          e,
          self.reqs_recvd as f64 / e,
          self.reqs_recvd - self.prev_print,
          self.total
        );
        self.prev_print = self.reqs_recvd;
        ctx.node.schedule_local_msg(
          Duration::from_millis(1000),
          ctx.local_interface(),
          LocalReportReceiverMsg::Print,
        );
      }
    }
  }

  async fn post_stop(
    &mut self,
    _: &ActorContext<BenchmarkTypes, LocalReportReceiverMsg>,
  ) {
    self.notify.send(()).await.unwrap();
  }
}

#[derive(AurumInterface)]
#[aurum(local)]
enum LocalIoTBusinessMsg {
  ReportReq(LocalRef<LocalReportReceiverMsg>),
}
struct LocalIoTBusiness {
  notify: Sender<()>,
}
#[async_trait]
impl Actor<BenchmarkTypes, LocalIoTBusinessMsg> for LocalIoTBusiness {
  async fn recv(
    &mut self,
    _: &ActorContext<BenchmarkTypes, LocalIoTBusinessMsg>,
    msg: LocalIoTBusinessMsg,
  ) {
    match msg {
      LocalIoTBusinessMsg::ReportReq(requester) => {
        //let items = (1..1000u64).collect_vec();
        let msg = LocalReportReceiverMsg::Report(true);
        requester.send(msg);
      }
    }
  }

  async fn post_stop(
    &mut self,
    _: &ActorContext<BenchmarkTypes, LocalIoTBusinessMsg>,
  ) {
    self.notify.send(()).await.unwrap();
  }
}

#[derive(AurumInterface, Serialize, Deserialize)]
enum ReportReceiverMsg {
  Tick(u64),
  Report(Vec<u64>),
  Print,
}
struct ReportReceiver {
  notify: Sender<()>,
  client: Socket,
  dest: Destination<BenchmarkTypes, IoTBusinessMsg>,
  req_timeout: Duration,
  reqs_sent: u64,
  reqs_recvd: u64,
  total: u128,
  start: Instant,
  prev_print: u64,
}
impl ReportReceiver {
  async fn req(
    &self,
    num: u64,
    ctx: &ActorContext<BenchmarkTypes, ReportReceiverMsg>,
  ) {
    let msg = IoTBusinessMsg::ReportReq(ctx.interface());
    ctx.node.udp_msg(&self.client, &self.dest, &msg).await;
    ctx.node.schedule_local_msg(
      self.req_timeout,
      ctx.local_interface(),
      ReportReceiverMsg::Tick(num),
    );
  }
}
#[async_trait]
impl Actor<BenchmarkTypes, ReportReceiverMsg> for ReportReceiver {
  async fn pre_start(
    &mut self,
    ctx: &ActorContext<BenchmarkTypes, ReportReceiverMsg>,
  ) {
    self.reqs_sent = 1;
    self.reqs_recvd = 0;
    self.req(self.reqs_sent, ctx).await;
    ctx.node.schedule_local_msg(
      Duration::from_millis(1000),
      ctx.local_interface(),
      ReportReceiverMsg::Print,
    );
  }

  async fn recv(
    &mut self,
    ctx: &ActorContext<BenchmarkTypes, ReportReceiverMsg>,
    msg: ReportReceiverMsg,
  ) {
    match msg {
      ReportReceiverMsg::Tick(num) => {
        if self.reqs_recvd < num {
          self.req(num, ctx).await;
        }
      }
      ReportReceiverMsg::Report(contents) => {
        self.reqs_recvd += 1;
        for x in contents {
          self.total += x as u128;
        }
        self.reqs_sent += 1;
        self.req(self.reqs_sent, ctx).await;
      }
      ReportReceiverMsg::Print => {
        let e = self.start.elapsed().as_secs_f64();
        println!(
          "Recvs: {}; Elapsed: {}; Rate; {}; Since Last: {}; Total: {}",
          self.reqs_recvd,
          e,
          self.reqs_recvd as f64 / e,
          self.reqs_recvd - self.prev_print,
          self.total
        );
        self.prev_print = self.reqs_recvd;
        ctx.node.schedule_local_msg(
          Duration::from_millis(1000),
          ctx.local_interface(),
          ReportReceiverMsg::Print,
        );
      }
    }
  }

  async fn post_stop(
    &mut self,
    _: &ActorContext<BenchmarkTypes, ReportReceiverMsg>,
  ) {
    self.notify.send(()).await.unwrap();
  }
}

#[derive(AurumInterface, Serialize, Deserialize)]
enum IoTBusinessMsg {
  ReportReq(ActorRef<BenchmarkTypes, ReportReceiverMsg>),
}
struct IoTBusiness {
  notify: Sender<()>,
}
#[async_trait]
impl Actor<BenchmarkTypes, IoTBusinessMsg> for IoTBusiness {
  async fn recv(
    &mut self,
    ctx: &ActorContext<BenchmarkTypes, IoTBusinessMsg>,
    msg: IoTBusinessMsg,
  ) {
    match msg {
      IoTBusinessMsg::ReportReq(requester) => {
        let items = (1..1000u64).collect_vec();
        let msg = ReportReceiverMsg::Report(items);
        ctx
          .node
          .udp_msg(&requester.socket, &requester.dest, &msg)
          .await;
      }
    }
  }

  async fn post_stop(
    &mut self,
    _: &ActorContext<BenchmarkTypes, IoTBusinessMsg>,
  ) {
    self.notify.send(()).await.unwrap();
  }
}
