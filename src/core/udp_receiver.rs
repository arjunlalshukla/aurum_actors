use crate::core::{
  DatagramHeader, MessageBuilder, Node, RegistryMsg, UnifiedBounds,
};
use std::collections::{hash_map::Entry, HashMap};
use std::convert::TryFrom;
use std::net::Ipv4Addr;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::unbounded_channel;
use tokio::task::JoinHandle;

const MSG_TIMEOUT: Duration = Duration::from_millis(1000);

pub(crate) async fn udp_receiver<U: UnifiedBounds>(node: Node<U>) {
  let mut recvd = HashMap::<u64, (JoinHandle<()>, MessageBuilder)>::new();
  let udp = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, node.socket().udp))
    .await
    .unwrap();
  let (tx, mut rx) = unbounded_channel::<u64>();
  let mut header_buf = [0u8; DatagramHeader::SIZE];
  loop {
    tokio::select! {
      res = udp.peek_from(&mut header_buf[..]) => {
        if res.is_err() {
          panic!("UDP peek failed!");
        }
        let header = match DatagramHeader::try_from(&header_buf[..]) {
          Ok(h) => h,
          _ => {
            udp.recv(&mut header_buf[..]).await.unwrap();
            continue;
          }
        };
        if header.max_seq_num == 0 {
          let mut mb = MessageBuilder::new(&header);
          mb.insert(&header, &udp).await;
          node.registry(RegistryMsg::Forward(mb));
        } else {
          match recvd.entry(header.msg_id) {
            Entry::Occupied(mut o) => {
              let hdl_mb = o.get_mut();
              hdl_mb.1.insert(&header, &udp).await;
              if hdl_mb.1.finished() {
                let (_, (hdl, mb)) = o.remove_entry();
                hdl.abort();
                node.registry(RegistryMsg::Forward(mb));
              }
            }
            Entry::Vacant(v) => {
              let mut mb = MessageBuilder::new(&header);
              mb.insert(&header, &udp).await;
              let tx = tx.clone();
              let hdl = node.rt().spawn(async move {
                tokio::time::sleep(MSG_TIMEOUT).await;
                tx.send(header.msg_id).unwrap();
              });
              v.insert((hdl, mb));
            }
          }
        }
      }
      msg_id = rx.recv() => {
        recvd.remove(&msg_id.unwrap());
      }
    }
  }
}
