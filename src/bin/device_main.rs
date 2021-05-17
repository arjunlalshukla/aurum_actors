use aurum::cluster::devices::{DeviceClient, DeviceClientConfig, DeviceServer};
use aurum::cluster::{Cluster, ClusterConfig, HBRConfig};
use aurum::core::{Host, Node, Socket};
use aurum::testkit::FailureConfigMap;
use aurum::{unify, AurumInterface};
use std::time::Duration;

unify!(BenchmarkTypes = DataCenterBusinessMsg | IoTBusinessMsg);

#[derive(AurumInterface)]
#[aurum(local)]
enum DataCenterBusinessMsg {}

#[derive(AurumInterface)]
#[aurum(local)]
enum IoTBusinessMsg {}

fn main() {
  // Exclude the command
  let mut args = std::env::args().skip(1);
  let mode = args.next().unwrap();
  let port = args.next().unwrap();
  println!("port: {}", port);
  let port = port.parse().unwrap();
  let interval = Duration::from_millis(args.next().unwrap().parse().unwrap());
  let mut fail_map = FailureConfigMap::default();
  fail_map.cluster_wide.drop_prob = args.next().unwrap().parse().unwrap();
  let lower = Duration::from_millis(args.next().unwrap().parse().unwrap());
  let upper = Duration::from_millis(args.next().unwrap().parse().unwrap());
  fail_map.cluster_wide.delay = Some((lower, upper));
  let host = Host::DNS("127.0.0.1".to_string());
  let seeds = args
    .map(|s| Socket::new(host.clone(), s.parse().unwrap(), 1001))
    .collect();
  let socket = Socket::new(host, port, 1001);
  let node = Node::<BenchmarkTypes>::new(socket, 1).unwrap();

  let name = "my-cool-device-cluster".to_string();
  let clr_cfg = ClusterConfig::default();
  let hbr_cfg = HBRConfig::default();

  match mode.as_str() {
    "server" => {
      let cluster = Cluster::new(
        &node,
        name.clone(),
        3,
        vec![],
        fail_map.clone(),
        clr_cfg,
        hbr_cfg,
      );
      DeviceServer::new(&node, cluster, vec![], name, fail_map);
    }
    "client" => {
      let mut cfg = DeviceClientConfig::default();
      cfg.seeds = seeds;
      cfg.initial_interval = interval;
      DeviceClient::new(&node, cfg, name, fail_map, vec![]);
    }
    _ => panic!("invalid mode {}", mode),
  }

  std::thread::sleep(Duration::from_secs(500));
}
