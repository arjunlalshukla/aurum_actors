#![warn(missing_docs)]

//! This crate implements a distributed actor model. Modelled after the widely-used [Akka] library,
//! `Aurum` looks to address some of its weaknesses, particularly in the typed interface for [Akka]
//! while giving higher performance. 
//! [Akka]: https://akka.io/

pub mod cluster;
pub mod core;
pub mod testkit;
extern crate aurum_macros;

pub use aurum_macros::{unify, AurumInterface};
