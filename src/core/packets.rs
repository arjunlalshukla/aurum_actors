use super::{serialize, Destination, UnifiedBounds, MAX_PACKET_SIZE};
use rand::Rng;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::net::SocketAddr;
use tokio::net::UdpSocket;

pub struct MessagePackets {
  msg_size: u32,
  dest_size: u16,
  max_seq_num: u16,
  buf: Vec<u8>,
}
impl MessagePackets {
  pub fn new<T: Serialize + DeserializeOwned, U: UnifiedBounds>(
    item: &T,
    dest: &Destination<U>,
  ) -> MessagePackets {
    let mut ser = serialize(item).unwrap();
    let msg_size = ser.len();
    ser.append(&mut serialize(dest).unwrap());
    MessagePackets {
      msg_size: msg_size as u32,
      dest_size: (ser.len() - msg_size) as u16,
      max_seq_num: (ser.len() / (MAX_PACKET_SIZE - DatagramHeader::SIZE))
        as u16,
      buf: ser,
    }
  }

  fn seq_slice(&self, seq_num: u16) -> &[u8] {
    let start = MAX_PACKET_SIZE * (seq_num as usize) - DatagramHeader::SIZE;
    let end = if self.max_seq_num == seq_num {
      self.buf.len()
    } else {
      start + MAX_PACKET_SIZE
    };
    &self.buf[start..end]
  }

  pub async fn send_to(self, socket: &UdpSocket, addr: &SocketAddr) {
    if self.buf.len() > (MAX_PACKET_SIZE - DatagramHeader::SIZE) * 0x10000 {
      panic!("Serialized item too large");
    }
    if self.max_seq_num != 0 {
      panic!("More than one packet not supported yet");
    }
    let mut first = if self.max_seq_num == 0 {
      vec![0u8; self.buf.len() + DatagramHeader::SIZE]
    } else {
      vec![0u8; MAX_PACKET_SIZE]
    };
    let mut header = DatagramHeader {
      msg_id: rand::thread_rng().gen::<u64>(),
      seq_num: 0,
      max_seq_num: 0,
      msg_size: self.msg_size as u32,
      dest_size: self.dest_size as u16,
    };
    header.put(&mut first[..DatagramHeader::SIZE]);
    let len = first.len();
    first[DatagramHeader::SIZE..]
      .copy_from_slice(&self.buf[..len - DatagramHeader::SIZE]);
    socket.send_to(&first, addr).await.unwrap();
  }
}

/*
We can't use Serde here because we need to know exactly how big the byte slice
is. Serde hides that as an implementation detail. Datagrams consist of 2
serialized objects concatenated: this header and the actual message. Without
knowing exactly how big the header is, we can't deserialize the message because
we don't know where it starts.
 */
#[derive(Debug, Eq, PartialEq)]
pub struct DatagramHeader {
  pub msg_id: u64,
  pub seq_num: u16,
  pub max_seq_num: u16,
  pub msg_size: u32,
  pub dest_size: u16,
}
// Serialization is big endian
impl DatagramHeader {
  pub const SIZE: usize = 18;
  pub fn put(&self, buf: &mut [u8]) {
    if buf.len() != Self::SIZE {
      panic!("Datagram ser buf: {}, expected {}", buf.len(), Self::SIZE);
    }
    buf[0] = (self.msg_id >> 56) as u8;
    buf[1] = (self.msg_id >> 48) as u8;
    buf[2] = (self.msg_id >> 40) as u8;
    buf[3] = (self.msg_id >> 32) as u8;
    buf[4] = (self.msg_id >> 24) as u8;
    buf[5] = (self.msg_id >> 16) as u8;
    buf[6] = (self.msg_id >> 8) as u8;
    buf[7] = self.msg_id as u8;
    buf[8] = (self.seq_num >> 8) as u8;
    buf[9] = self.seq_num as u8;
    buf[10] = (self.max_seq_num >> 8) as u8;
    buf[11] = self.max_seq_num as u8;
    buf[12] = (self.msg_size >> 24) as u8;
    buf[13] = (self.msg_size >> 16) as u8;
    buf[14] = (self.msg_size >> 8) as u8;
    buf[15] = self.msg_size as u8;
    buf[16] = (self.dest_size >> 8) as u8;
    buf[17] = self.dest_size as u8;
  }
}
impl From<&[u8]> for DatagramHeader {
  fn from(buf: &[u8]) -> Self {
    if buf.len() != Self::SIZE {
      panic!("Datagram de buf: {}, expected {}", buf.len(), Self::SIZE);
    }
    let mut ret = DatagramHeader {
      msg_id: 0,
      seq_num: 0,
      max_seq_num: 0,
      msg_size: 0,
      dest_size: 0,
    };
    ret.msg_id |= (buf[0] as u64) << 56;
    ret.msg_id |= (buf[1] as u64) << 48;
    ret.msg_id |= (buf[2] as u64) << 40;
    ret.msg_id |= (buf[3] as u64) << 32;
    ret.msg_id |= (buf[4] as u64) << 24;
    ret.msg_id |= (buf[5] as u64) << 16;
    ret.msg_id |= (buf[6] as u64) << 8;
    ret.msg_id |= buf[7] as u64;
    ret.seq_num |= (buf[8] as u16) << 8;
    ret.seq_num |= buf[9] as u16;
    ret.max_seq_num |= (buf[10] as u16) << 8;
    ret.max_seq_num |= buf[11] as u16;
    ret.msg_size |= (buf[12] as u32) << 24;
    ret.msg_size |= (buf[13] as u32) << 16;
    ret.msg_size |= (buf[14] as u32) << 8;
    ret.msg_size |= buf[15] as u32;
    ret.dest_size |= (buf[16] as u16) << 8;
    ret.dest_size |= buf[17] as u16;
    ret
  }
}

#[test]
fn test_datagram_header_serde() {
  let header = DatagramHeader {
    msg_id: 0x0f0e0d0c0b0a0908,
    seq_num: 0xea01,
    max_seq_num: 0x90f4,
    msg_size: 0x8b5d7015,
    dest_size: 0x8531,
  };
  let mut buf = [0u8; DatagramHeader::SIZE];
  header.put(&mut buf);
  assert_eq!(header, DatagramHeader::from(&buf[..]));
}
