#![allow(unused_imports, dead_code, unused_variables)]

use aurum::core::Node;
use aurum_macros::{unify, AurumInterface};
use std::env::args;

struct ClusterNode;

fn main() {
  let mut args = args().into_iter();
  let port = args.next().unwrap().parse::<u16>().unwrap();
  //let node = Node::new
}
