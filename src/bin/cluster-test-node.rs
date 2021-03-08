#![allow(unused_imports, dead_code, unused_variables)]

use aurum::core::{Host, Node, Socket};
use aurum_macros::{unify, AurumInterface};
use std::env::args;

struct ClusterNode;

unify!(ClusterNodeTypes = String);

fn main() {
  let mut args = args().into_iter();
  let port = args.next().unwrap().parse::<u16>().unwrap();
  let socket = Socket::new(Host::DNS("127.0.0.1".to_string()), port, 0);
  let node = Node::<ClusterNodeTypes>::new(socket.clone(), 1).unwrap();
}
