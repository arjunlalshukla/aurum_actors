use super::{ActorSignal, Case, Destination, LocalActorMsg, UnifiedBounds};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use rand::Rng;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashSet;
use std::convert::TryFrom;
use std::fmt::Debug;
use std::net::SocketAddr;
use tokio::net::UdpSocket;

const MAX_PACKET_SIZE: usize = DatagramHeader::SIZE * 2;

#[derive(Debug)]
pub enum DeserializeError<U: Debug> {
  IncompatibleInterface(U),
  Other(U),
}

pub fn serialize<T: Serialize + DeserializeOwned>(item: &T) -> Option<Vec<u8>> {
  serde_json::to_vec(item).ok()
}

pub fn deserialize<T: Serialize + DeserializeOwned>(bytes: &[u8]) -> Option<T> {
  serde_json::from_slice::<T>(bytes).ok()
}

pub fn deserialize_msg<U, S, I>(
  interface: U,
  intp: Interpretations,
  bytes: &[u8],
) -> Result<LocalActorMsg<S>, DeserializeError<U>>
where
  U: Case<S> + Case<I> + UnifiedBounds,
  S: From<I>,
  I: Serialize + DeserializeOwned,
{
  match intp {
    Interpretations::Message => match deserialize::<I>(bytes) {
      Some(res) => Result::Ok(LocalActorMsg::Msg(S::from(res))),
      None => Result::Err(DeserializeError::Other(interface)),
    },
    Interpretations::Signal => match deserialize::<ActorSignal>(bytes) {
      Some(res) => Result::Ok(LocalActorMsg::Signal(res)),
      None => Result::Err(DeserializeError::Other(interface)),
    },
  }
}

pub struct MessagePackets {
  msg_size: u32,
  dest_size: u16,
  max_seq_num: u16,
  buf: Vec<u8>,
  intp: Interpretations,
}
impl MessagePackets {
  pub fn new<T: Serialize + DeserializeOwned, U: UnifiedBounds>(
    item: &T,
    intp: Interpretations,
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
      intp: intp,
    }
  }

  pub async fn send_to(mut self, socket: &UdpSocket, addr: &SocketAddr) {
    if self.buf.len() > (MAX_PACKET_SIZE - DatagramHeader::SIZE) * 0x10000 {
      panic!("Serialized item too large");
    }
    let mut first = if self.max_seq_num == 0 {
      vec![0u8; self.buf.len() + DatagramHeader::SIZE]
    } else {
      vec![0u8; MAX_PACKET_SIZE]
    };
    let mut header = DatagramHeader {
      msg_id: rand::thread_rng().gen::<u64>(),
      seq_num: 0,
      max_seq_num: self.max_seq_num,
      msg_size: self.msg_size as u32,
      dest_size: self.dest_size as u16,
      intp: self.intp,
    };
    header.put(&mut first[..DatagramHeader::SIZE]);
    let len = first.len();
    first[DatagramHeader::SIZE..]
      .copy_from_slice(&self.buf[..len - DatagramHeader::SIZE]);
    socket.send_to(&first, addr).await.unwrap();
    for i in 1..=self.max_seq_num {
      header.seq_num = i;
      let start = header.seq_num as usize
        * (MAX_PACKET_SIZE - DatagramHeader::SIZE)
        - DatagramHeader::SIZE;
      let end = std::cmp::min(start + MAX_PACKET_SIZE, self.buf.len());
      let slice = &mut self.buf[start..end];
      header.put(&mut slice[..DatagramHeader::SIZE]);
      socket.send_to(&slice, addr).await.unwrap();
    }
  }
}

pub struct MessageBuilder {
  msg_size: u32,
  max_seq_num: u16,
  seqs_recvd: HashSet<u16>,
  buf: Vec<u8>,
  pub intp: Interpretations,
}
impl MessageBuilder {
  pub fn new(header: &DatagramHeader) -> MessageBuilder {
    MessageBuilder {
      msg_size: header.msg_size,
      max_seq_num: header.max_seq_num,
      seqs_recvd: HashSet::new(),
      buf: vec![
        0u8;
        header.msg_size as usize
          + header.dest_size as usize
          + DatagramHeader::SIZE
      ],
      intp: header.intp,
    }
  }

  pub async fn insert(&mut self, header: &DatagramHeader, socket: &UdpSocket) {
    if self.seqs_recvd.contains(&header.seq_num) {
      return;
    }
    self.seqs_recvd.insert(header.seq_num);
    let start = if header.seq_num == 0 {
      0
    } else {
      header.seq_num as usize * (MAX_PACKET_SIZE - DatagramHeader::SIZE)
    };
    let end = std::cmp::min(start + MAX_PACKET_SIZE, self.buf.len());
    let slice = &mut self.buf[start..end];
    let mut header_buf = [0u8; DatagramHeader::SIZE];
    header_buf[..].copy_from_slice(&slice[..DatagramHeader::SIZE]);
    let _bytes = socket.recv(slice).await.unwrap();
    &slice[..DatagramHeader::SIZE].copy_from_slice(&header_buf[..]);
  }

  pub fn finished(&self) -> bool {
    self.seqs_recvd.len() == (self.max_seq_num as usize + 1)
  }

  pub fn dest(&self) -> &[u8] {
    &self.buf[DatagramHeader::SIZE + self.msg_size as usize..]
  }

  pub fn msg(&self) -> &[u8] {
    &self.buf
      [DatagramHeader::SIZE..DatagramHeader::SIZE + self.msg_size as usize]
  }
}

#[derive(
  Copy, Clone, Debug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive,
)]
#[repr(u8)]
pub enum Interpretations {
  Message = 0,
  Signal = 1,
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
  pub intp: Interpretations,
}
// Serialization is big endian
impl DatagramHeader {
  pub const SIZE: usize = 19;
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
    buf[18] = self.intp.into();
  }
}
impl TryFrom<&[u8]> for DatagramHeader {
  type Error = ();
  fn try_from(buf: &[u8]) -> Result<Self, ()> {
    if buf.len() != Self::SIZE {
      return Err(());
    }
    let mut ret = DatagramHeader {
      msg_id: 0,
      seq_num: 0,
      max_seq_num: 0,
      msg_size: 0,
      dest_size: 0,
      intp: Interpretations::try_from(buf[18]).map_err(|_| ())?,
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
    if ret.seq_num > ret.max_seq_num || ret.dest_size == 0 {
      Err(())
    } else {
      Ok(ret)
    }
  }
}

#[test]
fn test_datagram_header_serde() {
  let header = DatagramHeader {
    msg_id: 0x0f0e0d0c0b0a0908,
    seq_num: 0x90f4,
    max_seq_num: 0xea01,
    msg_size: 0x8b5d7015,
    dest_size: 0x8531,
    intp: Interpretations::Message,
  };
  let mut buf = [0u8; DatagramHeader::SIZE];
  header.put(&mut buf);
  assert_eq!(Ok(header), DatagramHeader::try_from(&buf[..]));
}
