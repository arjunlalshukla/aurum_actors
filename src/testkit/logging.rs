use crate as aurum;
use crate::core::{Actor, ActorContext, Case, UnifiedBounds};
use crate::AurumInterface;
use async_trait::async_trait;
use std::fmt::Display;
use LogLevel::*;
use LoggerMsg::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
  Trace,
  Debug,
  Info,
  Warn,
  Error,
  Fatal,
  Off,
}
impl LogLevel {
  pub const MIN: LogLevel = LogLevel::Trace;

  pub const fn caps(&self) -> &'static str {
    match self {
      Trace => "TRACE",
      Debug => "DEBUG",
      Info => "INFO",
      Warn => "WARN",
      Error => "ERROR",
      Fatal => "FATAL",
      Off => "OFF",
    }
  }
}

pub enum LogSpecial {
  SentBytes(u64),
  RecvdBytes(u64),
}

#[derive(AurumInterface)]
#[aurum(local)]
pub enum LoggerMsg {
  Log(
    LogLevel,
    &'static str,
    u32,
    u32,
    Box<dyn Display + Send + 'static>,
  ),
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
      Log(level, file, line, column, log) => {
        if level >= self.level {
          println!(
            "{} {}:{}:{} - port {} - {}",
            level.caps(),
            file,
            line,
            column,
            ctx.node.socket().udp,
            log
          );
        }
      }
      SetLevel(level) => self.level = level,
      Special(s) => match s {
        LogSpecial::SentBytes(b) => self.bytes_sent += b,
        LogSpecial::RecvdBytes(b) => self.bytes_recvd += b,
      },
    }
  }
}

#[macro_export]
macro_rules! log {
  ($env_level:expr, $node:expr, $msg_level:expr, $msg:expr) => {
    if $msg_level >= $env_level {
      $node.log(aurum::testkit::LoggerMsg::Log(
        $msg_level,
        std::file!(),
        std::line!(),
        std::column!(),
        std::boxed::Box::new($msg),
      ));
    }
  };
}

#[macro_export]
macro_rules! trace {
  ($env_level:expr, $node:expr, $msg:expr) => {
    aurum::log!($env_level, $node, aurum::testkit::LogLevel::Trace, $msg);
  };
}

#[macro_export]
macro_rules! debug {
  ($env_level:expr, $node:expr, $msg:expr) => {
    aurum::log!($env_level, $node, aurum::testkit::LogLevel::Debug, $msg);
  };
}

#[macro_export]
macro_rules! info {
  ($env_level:expr, $node:expr, $msg:expr) => {
    aurum::log!($env_level, $node, aurum::testkit::LogLevel::Info, $msg);
  };
}

#[macro_export]
macro_rules! warn {
  ($env_level:expr, $node:expr, $msg:expr) => {
    aurum::log!($env_level, $node, aurum::testkit::LogLevel::Warn, $msg);
  };
}

#[macro_export]
macro_rules! error {
  ($env_level:expr, $node:expr, $msg:expr) => {
    aurum::log!($env_level, $node, aurum::testkit::LogLevel::Error, $msg);
  };
}

#[macro_export]
macro_rules! fatal {
  ($env_level:expr, $node:expr, $msg:expr) => {
    aurum::log!($env_level, $node, aurum::testkit::LogLevel::Fatal, $msg);
  };
}
