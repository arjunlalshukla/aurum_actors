use crate::core::{
  Case, DatagramHeader, MessageBuilder, Node, RegistryMsg, UnifiedBounds,
};
use std::collections::{hash_map::Entry, HashMap};
use std::convert::TryFrom;
use std::net::Ipv4Addr;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::unbounded_channel;
use tokio::task::JoinHandle;

const MSG_TIMEOUT: Duration = Duration::from_millis(1000);

pub(crate) async fn udp_receiver<U>(node: Node<U>)
where
  U: UnifiedBounds + Case<RegistryMsg<U>>,
{
  let mut recvd =
    HashMap::<u64, (Option<JoinHandle<()>>, MessageBuilder)>::new();
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
        let hdl_mb = match recvd.entry(header.msg_id) {
          Entry::Occupied(o) => {
            let hdl_mb = o.into_mut();
            hdl_mb.1.insert(&header, &udp).await;
            hdl_mb
          }
          Entry::Vacant(v) => {
            let mut mb = MessageBuilder::new(&header);
            mb.insert(&header, &udp).await;
            v.insert((None, mb))
          }
        };
        if hdl_mb.1.finished() {
          let hdl_mb = recvd.remove(&header.msg_id).unwrap();
          hdl_mb.0.iter().for_each(|x| x.abort());
          node.registry(RegistryMsg::Forward(hdl_mb.1));
        } else if hdl_mb.0.is_none() {
          let tx = tx.clone();
          hdl_mb.0 = Some(node.node.rt.spawn(async move {
            tokio::time::sleep(MSG_TIMEOUT).await;
            tx.send(header.msg_id).unwrap();
          }));
        }
      }
      msg_id = rx.recv() => {
        recvd.remove(&msg_id.unwrap());
      }
    }
  }
}
