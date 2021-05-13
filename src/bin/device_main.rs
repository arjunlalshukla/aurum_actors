use aurum::cluster::{Cluster, ClusterConfig, HBRConfig};
use aurum::cluster::devices::{DeviceClient, DeviceClientConfig, DeviceServer};
use aurum::core::{Host, Node, Socket};
use aurum::test_commons::ClusterNodeTypes;
use aurum::testkit::FailureConfigMap;
use itertools::Itertools;
use std::time::Duration;

fn main() {
  // Exclude the command
  let mut args = std::env::args().skip(1);
  let mode = args.next().unwrap();
  let port = args.next().unwrap();
  println!("port: {}", port);
  let port = port.parse().unwrap();
  let interval = Duration::from_millis(args.next().unwrap().parse().unwrap());
  let host = Host::DNS("127.0.0.1".to_string());
  let seeds = args.map(|s| Socket::new(host.clone(), s.parse().unwrap(), 1001)).collect_vec();
  let socket = Socket::new(host, port, 1001);
  let node = Node::<ClusterNodeTypes>::new(socket, 1).unwrap();

  let name = "my-cool-device-cluster".to_string();
  let mut fail_map = FailureConfigMap::default();
  fail_map.cluster_wide.drop_prob = 0.5;
  fail_map.cluster_wide.delay =
    Some((Duration::from_millis(20), Duration::from_millis(50)));
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
      DeviceClient::new(&node, interval, cfg, name, fail_map);
    }
    _ => panic!("invalid mode {}", mode),
  }

  std::thread::sleep(Duration::from_secs(500));
}
