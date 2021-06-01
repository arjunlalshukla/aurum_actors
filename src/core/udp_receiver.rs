use crate as aurum;
use crate::core::{
  DatagramHeader, MessageBuilder, Node, RegistryMsg, UnifiedType, LOG_LEVEL,
};
use crate::{fatal, trace};
use std::collections::{hash_map::Entry, HashMap};
use std::convert::TryFrom;
use std::time::Duration;
use tokio::sync::mpsc::unbounded_channel;
use tokio::task::JoinHandle;

const MSG_TIMEOUT: Duration = Duration::from_millis(1000);

pub(crate) async fn udp_receiver<U: UnifiedType>(node: Node<U>) {
  let mut recvd = HashMap::<u64, (JoinHandle<()>, MessageBuilder)>::new();
  let (tx, mut rx) = unbounded_channel::<u64>();
  let mut header_buf = [0u8; DatagramHeader::SIZE];
  loop {
    tokio::select! {
      res = node.udp_socket().peek_from(&mut header_buf[..]) => {
        if res.is_err() {
          fatal!(LOG_LEVEL, &node, format!("UDP socket read failed"));
          return;
        }
        let header = match DatagramHeader::try_from(&header_buf[..]) {
          Ok(h) => h,
          _ => {
            node.udp_socket().recv(&mut header_buf[..]).await.unwrap();
            continue;
          }
        };
        if header.max_seq_num == 0 {
          let mut mb = MessageBuilder::new(&header);
          mb.insert(&header, node.udp_socket()).await;
          node.registry(RegistryMsg::Forward(mb));
        } else {
          match recvd.entry(header.msg_id) {
            Entry::Occupied(mut o) => {
              let hdl_mb = o.get_mut();
              hdl_mb.1.insert(&header, node.udp_socket()).await;
              if hdl_mb.1.finished() {
                let (_, (hdl, mb)) = o.remove_entry();
                hdl.abort();
                node.registry(RegistryMsg::Forward(mb));
              }
            }
            Entry::Vacant(v) => {
              let mut mb = MessageBuilder::new(&header);
              mb.insert(&header, node.udp_socket()).await;
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
        let id = msg_id.unwrap();
        trace!(LOG_LEVEL, &node, format!("dropping msg with id: {}", id));
        recvd.remove(&msg_id.unwrap());
      }
    }
  }
}
