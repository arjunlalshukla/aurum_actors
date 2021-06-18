use crate::core::{
  serialize, ActorSignal, Case, DatagramHeader, Destination, Interpretations, UnifiedType,
};
use rand::random;
use serde::{de::DeserializeOwned, Serialize};
use std::net::SocketAddr;
use tokio::net::UdpSocket;

pub struct UdpSerial {
  bytes: Vec<u8>,
}
impl UdpSerial {
  // const MAX_SAFE_PAYLOAD: usize = 508;
  #[allow(dead_code)]
  const MAX_UDP_PAYLOAD: usize = 65507;
  pub const PACKET_SIZE: usize = 75;
  const PAYLOAD_PER_PACKET: usize = Self::PACKET_SIZE - DatagramHeader::SIZE;

  pub fn packets(&self) -> usize {
    self.bytes.len() / Self::PACKET_SIZE - (self.bytes.len() % Self::PACKET_SIZE == 0) as usize + 1
  }

  pub fn len(&self) -> usize {
    self.bytes.len()
  }

  pub fn payload_len(&self) -> usize {
    self.len() - DatagramHeader::SIZE * self.packets()
  }

  pub fn msg<U, I>(dest: &Destination<U, I>, item: &I) -> Self
  where
    U: UnifiedType + Case<I>,
    I: Serialize + DeserializeOwned,
  {
    Self::new(item, Interpretations::Message, dest)
  }

  pub fn sig<U, I>(dest: &Destination<U, I>, sig: &ActorSignal) -> Self
  where
    U: UnifiedType + Case<I>,
    I: Serialize + DeserializeOwned,
  {
    Self::new(sig, Interpretations::Signal, dest)
  }

  fn new<T: Serialize + DeserializeOwned, U: UnifiedType + Case<I>, I>(
    item: &T,
    intp: Interpretations,
    dest: &Destination<U, I>,
  ) -> Self {
    let mut item = serialize(item).unwrap();
    let mut dest = serialize(dest.untyped()).unwrap();
    let total = item.len() + dest.len();
    let max_seq_num = (total - 1) / Self::PAYLOAD_PER_PACKET;
    //println!("max_seq_num = {}, item = {}, dest = {}", max_seq_num, item.len(), dest.len());
    let mut buf = Vec::with_capacity(total + (max_seq_num + 1) * DatagramHeader::SIZE);
    let mut header = DatagramHeader {
      msg_id: random(),
      seq_num: 0,
      max_seq_num: max_seq_num as u16,
      msg_size: item.len() as u32,
      dest_size: dest.len() as u16,
      intp: intp,
    };
    item.append(&mut dest);
    let mut idx = 0usize;
    let mut header_buf = [0u8; DatagramHeader::SIZE];
    for _ in 0..max_seq_num {
      header.put(&mut header_buf[..]);
      buf.extend_from_slice(&header_buf);
      buf.extend_from_slice(&item[idx..idx + Self::PAYLOAD_PER_PACKET]);
      idx += Self::PAYLOAD_PER_PACKET;
      header.seq_num += 1;
    }
    header.put(&mut header_buf[..]);
    buf.extend_from_slice(&header_buf);
    buf.extend_from_slice(&item[idx..]);
    Self {
      bytes: buf,
    }
  }

  pub(in crate::core) async fn send(&self, socket: &UdpSocket, addr: &SocketAddr) {
    let mut idx = 0usize;
    for _ in 1..self.packets() {
      let next = idx + Self::PACKET_SIZE;
      socket.send_to(&self.bytes[idx..next], addr).await.unwrap();
      idx = next;
    }
    socket.send_to(&self.bytes[idx..], addr).await.unwrap();
  }
}

// #[cfg(test)]
// mod test {
//   use crate as aurum;
//   use crate::{unify, AurumInterface};
//   use serde::{Serialize, Deserialize};
//   unify!(pub UdpSerialTestTypes = ForgeMsg; Vec<u8>);
//   #[derive(AurumInterface, Serialize, Deserialize)]
//   pub enum ForgeMsg {
//     #[aurum]
//     Bytes(Vec<u8>)
//   }
// }

// #[cfg(test)]
// use test::*;

// #[test]
// fn test_one_edge() {
//   let payload: Vec<u8> = (0..1000).map(|_| random()).collect();
//   let dest = Destination::<UdpSerialTestTypes, Vec<u8>>::new::<ForgeMsg>("foo".to_string());
//   let ser = UdpSerial::msg(&dest, &payload);
//   assert_eq!(ser.packets(), ser.payload_len());
// }
