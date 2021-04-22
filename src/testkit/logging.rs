use crate as aurum;
use crate::core::{Actor, ActorContext, Case, UnifiedBounds};
use crate::AurumInterface;
use async_trait::async_trait;
use LoggerMsg::*;

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
  Trace,
  Debug,
  Info,
  Warn,
  Error,
  Fatal,
  Off
}
impl LogLevel {
  pub const MIN: LogLevel = LogLevel::Trace;
}

pub enum LogSpecial {
  SentBytes(u64),
  RecvdBytes(u64),
}

#[derive(AurumInterface)]
#[aurum(local)]
pub enum LoggerMsg {
  Log(LogLevel, Box<dyn ToString + Send + 'static>),
  SetLevel(LogLevel),
  Special(LogSpecial),
}

pub struct Logger {
  bytes_sent: u64,
  bytes_recvd: u64,
  level: LogLevel,
}
impl Logger {
  pub fn new(level: LogLevel) -> Self {
    Logger {
      bytes_sent: 0,
      bytes_recvd: 0,
      level: level,
    }
  }
}
#[async_trait]
impl<U: Case<LoggerMsg> + UnifiedBounds> Actor<U, LoggerMsg> for Logger {
  async fn recv(&mut self, ctx: &ActorContext<U, LoggerMsg>, msg: LoggerMsg) {
    match msg {
      Log(level, s) => {
        if level <= self.level {
          println!("{}", s.to_string());
        }
      }
      SetLevel(level) => self.level = level,
      Special(s) => {
        match s {
          LogSpecial::SentBytes(b)  => self.bytes_sent += b,
          LogSpecial::RecvdBytes(b) => self.bytes_recvd += b
        }
      }
    }
  }
}
