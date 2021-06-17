use crate::core::{
  serialize, ActorSignal, Case, DatagramHeader, Destination, Interpretations, UnifiedType,
};
use serde::{de::DeserializeOwned, Serialize};
use std::{cmp::max, io::Write};

// const MAX_SAFE_PAYLOAD: usize = 508;
const MAX_UDP_PAYLOAD: usize = 65507;
const MAX_PACKET_SIZE: usize = MAX_UDP_PAYLOAD;
const MSG_BYTES_PER_PACKET: usize = MAX_PACKET_SIZE - DatagramHeader::SIZE;

static EMPTY_HEADER: [u8; DatagramHeader::SIZE] = [0u8; DatagramHeader::SIZE];

pub struct UdpSerial {
  bytes: Vec<u8>,
}
impl UdpSerial {
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
    // let item = serialize(item).unwrap();
    // let dest = serialize(dest.untyped()).unwrap();
    // let total = item.len() + dest.len();
    // let max_seq_num = total / MSG_BYTES_PER_PACKET;
    // let mut buf = Vec::with_capacity(MAX_PACKET_SIZE * max_seq_num + DatagramHeader::SIZE + total%MSG_BYTES_PER_PACKET);
    // let mut header = DatagramHeader {
    //   msg_id: rand::random(),
    //   seq_num: 0,
    //   max_seq_num: max_seq_num as u16,
    //   msg_size: item.len() as u32,
    //   dest_size: dest.len() as u16,
    //   intp: intp,
    // };

    Self {
      bytes: vec![],
    }
  }
}
/*
struct UdpBufWriter<'a> {
  buf: &'a mut Vec<u8>,
}
impl Write for UdpBufWriter<'_> {
  fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
    let to_end = MAX_PACKET_SIZE - (self.buf.len() % MAX_PACKET_SIZE);

    Ok(0)
  }

  fn flush(&mut self) -> std::io::Result<()> {
    Ok(())
  }
}
*/
