#![allow(dead_code, unused_imports, unused_variables, unused_mut)]
use aurum::core::{Host, Socket};
use serde::{Deserialize, Serialize};
use serde_cbor::{from_slice, to_vec};
use std::time::{Duration, Instant};
use tokio::runtime::{Builder, Runtime};
use tokio::sync::oneshot::{channel, Sender};
use std::net::Ipv4Addr;
use tokio::net::UdpSocket;
use tokio::task::JoinHandle;

const CLUSTER_NAME: &'static str = "my-cool-device-cluster";

fn main() {
  // Exclude the command
  let mut args = std::env::args().skip(1);
  let threads = args.next().unwrap().parse().unwrap();
  let host = args.next().unwrap();
  let port = args.next().unwrap().parse().unwrap();
  let socket = Socket::new(Host::from(host), port, 0);
  let mode = args.next().unwrap();
  let host = args.next().unwrap();
  let port = args.next().unwrap().parse().unwrap();
  let target = Socket::new(Host::from(host), port, 0);
  let (tx, mut rx) = channel();

  let rt = Builder::new_multi_thread()
      .enable_io()
      .enable_time()
      .worker_threads(threads)
      .thread_name("tokio-thread")
      .thread_stack_size(3 * 1024 * 1024)
      .build()
      .unwrap();

  match mode.as_str() {
    "server" => {
      rt.spawn(recvr(tx, socket, target));
    }
    "client" => (),
    _ => panic!("invalid mode {}", mode),
  }

  rt.block_on(rx).unwrap();
}

async fn recvr(notify: Sender<()>, socket: Socket, target: Socket) {
  let mut reqs_recvd = 0u64;
  let mut total = 0u128;
  let mut start = Instant::now();
  let udp = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, socket.udp)).await.unwrap();
  let mut buf = [0u8; 0xffff];
  loop {
    let addr = socket.as_udp_addr().await.unwrap().into_iter().next().unwrap();
    let sender = tokio::net::UdpSocket::bind((std::net::Ipv4Addr::UNSPECIFIED, 0))
      .await
      .unwrap();
    let a = to_vec(&true).unwrap();
    sender.send_to(&a[..], addr).await.unwrap();
    let bytes = udp.recv(&mut buf[..]).await.unwrap();
    let msg: Vec<u64> = from_slice(&buf[..bytes]).unwrap();
    for x in msg {
      total += x as u128;
    }
    reqs_recvd += 1;
  }
}
