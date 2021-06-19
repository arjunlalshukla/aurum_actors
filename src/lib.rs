#![warn(missing_docs)]

//! [`Aurum`](crate) implements a distributed actor model. Its purpose is to make distributed programming
//! easy. [`Aurum`](crate) provides support for clustering, [CRDT] sharing among cluster members and [IoT]
//! device tracking. It has excellent type safety and full [serde] support.
//!
//! # Why Would You Want to Use [`Aurum`](crate)?
//! ### Typed Actors
//! Actor references are typed, meaning they can only receive messages of a specific type. Untyped
//! actor references can receive messages of any type. Typed references add safety by turning some
//! runtime errors into compile time errors, and eliminate the need for catch-all match branches
//! that handle invalid messages. They also make code easier to read. Type information gives extra
//! hints of an actor’s role in the program. Actor references are statically typed. The type of
//! messages they receive must be defined at compile time. Generics are used to communicate the
//! types of messages. Most distributed actor model implementations use untyped actors.
//! 
//! ### Actor Interfaces
//! It is not unusual for someone to want to store actor references in a data structure (for
//! instance, to keep track of subscribers to events). Collections in Rust must be homogeneous,
//! but there may be many different kinds of actors who want to subscribe to the same events.
//! You cannot keep actor references receiving different types in the same data structure, nor is
//! creating a different structure for every possible type desirable, nor do we want to create
//! separate actors that relay event messages to the real subscriber under its preferred type. Actor
//! interfaces allow the programmer to define a subset of message types receivable by an actor, and
//! create actor references which receive only that subset.
//! 
//! Actor interfaces are also useful if some of the messages an actor receives are serializable, and
//! others are not. You can define a serializable subset, and create actor references that receive 
//! that subset. [`Aurum`](crate) provides annotations on message types to create automatic
//! translations between subtypes and root message types.
//! 
//! ### Forgeable References
//! [`Aurum`](crate) was created to address fill niches in existing actor models like [Akka]. While
//! [Akka] has support for both typed and untyped actors, typed actor references in [Akka] are not
//! forgeable (i.e. they cannot be created independently). With [Akka]'s typed actors, you need a
//! complex discovery protocol built underneath to acquire a reference to the actor for sending
//! messages.
//!
//! A single system may have many actors running on it, so these actors receive their messages on
//! same network socket. Messages are then routed locally using a registry. Each actor is registered
//! on their socket with a unique identifier. Our problem comes from [Akka] using strings for its
//! actor identifiers. Strings do not contain any type information about the actor, so how do we
//! know what type to give our reference we just forged? It's not safe to just guess.  [`Aurum`](crate) fixes
//! this problem by embedding type information within actor identifiers. Identifiers consist of both
//! a string and a type id for discriminating between identifiers with the same string name. The
//! type safety of forged references is enforced at compile time. None of this type safety comes at
//! the cost of performance.
//!
//! # Modules and Features
//! [`Aurum`](crate) is a heavily modularized piece of software. [`cluster`](crate::cluster) is built
//! on top of [`core`](crate::core) with no internal access. If you plan on building a library on top
//! of [`Aurum`](crate), [`cluster`](crate::cluster) is a good reference.
//! - [`cluster`](crate::cluster): Single data center clustering.
//!   - [`crdt`](crate::cluster::crdt): Intra-cluster CRDT sharing.
//!   - [`devices`](crate::cluster::devices): External node management. Useful for IoT.
//! - [`core`](crate::core): The basics. Start here if you're new to [`Aurum`](crate) and want to
//! learn how to use it.
//! - [`testkit`](crate::testkit): Tools for testing actors and injecting failures.
//! 
//!
//! [actor model]: https://en.wikipedia.org/wiki/Actor_model
//! [serde]: https://docs.serde.rs/serde/
//! [CRDT]: https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type
//! [Akka]: https://akka.io/
//! [IoT]: https://en.wikipedia.org/wiki/Internet_of_things

pub mod cluster;
pub mod core;
pub mod testkit;
extern crate aurum_macros;

pub use aurum_macros::{unify, AurumInterface};
