#![allow(dead_code, unused_variables, unused_mut, unused_imports)]
use crate::core::{Case, DatagramHeader, Node, RegistryMsg, UnifiedBounds};
use itertools::Itertools;
use std::collections::{hash_map::Entry, HashMap, HashSet};
use tokio::sync::oneshot::Receiver;

struct Message {
  seq_length: u16,
  seqs_recvd: HashSet<u16>,
  msg: Vec<u8>,
}

pub(crate) async fn udp_receiver<
  Unified: UnifiedBounds + Case<RegistryMsg<Unified>>,
>(
  node: Receiver<Node<Unified>>,
) {
  let node = node.await.unwrap();
  println!("Started udp receiver");
  let mut recvd = HashMap::<u64, Message>::new();
  let udp = tokio::net::UdpSocket::bind(
    node
      .socket()
      .as_udp_addr()
      .await
      .unwrap()
      .into_iter()
      .exactly_one()
      .unwrap(),
  )
  .await
  .unwrap();
  let mut header_buf = [0u8; DatagramHeader::SIZE];
  loop {
    let (len, _) = udp.peek_from(&mut header_buf[..]).await.unwrap();
    if len != DatagramHeader::SIZE {
      udp.recv(&mut header_buf[..]).await.unwrap();
      continue;
    }
    let header = DatagramHeader::from(&header_buf[..]);
    let mut msg = vec![
      0u8;
      header.msg_size as usize
        + header.dest_size as usize
        + DatagramHeader::SIZE
    ];
    let bytes = udp.recv(&mut msg[..]).await.unwrap();
    let expected = DatagramHeader::SIZE as usize
      + header.msg_size as usize
      + header.dest_size as usize;
    if bytes != expected {
      panic!(
        "Expected {} bytes, got {} bytes for {:?}",
        expected, bytes, header
      );
    }
    node.registry(RegistryMsg::Forward(header, msg));
  }
}

/*
let mut msg = match recvd.entry(header.msg_id) {
  Entry::Occupied(o ) => (),
  Entry::Vacant(v) => {
    v.insert(Message {
      seq_length: header.max_seq_num,
      msg: vec![0u8; header.msg_size as usize + DatagramHeader::SIZE],
      seqs_recvd: {
        let mut set = HashSet::new();
        set.insert(header.seq_num);
        set
      }
    });
  },
};
*/
