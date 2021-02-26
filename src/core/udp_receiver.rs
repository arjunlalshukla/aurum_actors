#![allow(dead_code, unused_variables, unused_mut, unused_imports)]
use crate::core::{
  Case, DatagramHeader, MessageBuilder, Node, RegistryMsg, UnifiedBounds,
};
use itertools::Itertools;
use std::net::Ipv4Addr;
use std::{
  borrow::BorrowMut,
  collections::{hash_map::Entry, HashMap, HashSet},
};
use tokio::net::UdpSocket;
use tokio::sync::oneshot::Receiver;

pub(crate) async fn udp_receiver<
  Unified: UnifiedBounds + Case<RegistryMsg<Unified>>,
>(
  node: Receiver<Node<Unified>>,
) {
  let node = node.await.unwrap();
  println!("Started udp receiver");
  let mut recvd = HashMap::<u64, MessageBuilder>::new();
  let udp = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, node.socket().udp))
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
    let msg = match recvd.entry(header.msg_id) {
      Entry::Occupied(mut o) => {
        let mb = o.into_mut();
        mb.insert(&header, &udp).await;
        mb
      }
      Entry::Vacant(v) => {
        let mut mb = MessageBuilder::new(&header);
        mb.insert(&header, &udp).await;
        v.insert(mb)
      }
    };
    if msg.finished() {
      node
        .registry(RegistryMsg::Forward(recvd.remove(&header.msg_id).unwrap()));
    }
  }
}
