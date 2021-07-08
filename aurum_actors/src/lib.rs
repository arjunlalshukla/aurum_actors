// #![warn(missing_docs)]

//! [`Aurum`](crate) implements a distributed actor model. Its purpose is to make distributed programming
//! easy. [`Aurum`](crate) provides support for clustering, [CRDT] sharing among cluster members and [IoT]
//! device tracking. It has excellent type safety and full [serde] support.
//!
//! # Prerequisite Knowledge
//! We suggest having this knowledge before starting here:
//! - A basic understanding of the [actor model]
//! - Some knowledge of [serialization] and [serde]
//! - [`async/await`] in Rust
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
//! [`async/await`]: https://rust-lang.github.io/async-book/
//! [serialization]: https://en.wikipedia.org/wiki/Serialization
//! [actor model]: https://en.wikipedia.org/wiki/Actor_model
//! [serde]: https://docs.serde.rs/serde/
//! [CRDT]: https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type
//! [Akka]: https://akka.io/
//! [IoT]: https://en.wikipedia.org/wiki/Internet_of_things

pub mod cluster;
pub mod core;
pub mod testkit;
extern crate aurum_actors_macros;

/// Creates a [`UnifiedType`](crate::core::UnifiedType) and implements traits for it.
///
/// This macro is central to how [`Aurum`](crate) functions. Because we have actor interfaces, there
/// are multiple possible interpretations of a sequence of bytes. In order to deserialize messages
/// correctly, we need to know how to interpret the bytes. We need to know what type was serialized.
/// Information on what type the message is must be included within the message.
/// [`UnifiedType`](crate::core::UnifiedType) instances are also used to safely deserialize
/// [`Destination`](crate::core::Destination) instances, which contain a
/// [`UnifiedType`](crate::core::UnifiedType) instance. Generic type information does not serialize,
/// so if a [`Destination`](crate::core::Destination) is deserialized and interpreted with a
/// generic type, we have to make sure that the [`ActorId`](crate::core::ActorId) and interface
/// match that generic type as well as match each other, to make sure we do not create a
/// [`Destination`](crate::core::Destination) that is invalid according to our type system.
///
/// Rust's [`TypeId`](std::any::TypeId) is not serializable, and does not come with any guarantees
/// at all. We had to create our own system of reflection to fix this problem, but it is fairly easy
/// to use. The [`UnifiedType`](crate::core::UnifiedType) created by this macro is an enum, whose
/// variant represent all possible root message types and actor interfaces usable in an application.
/// A type that is not in the [`UnifiedType`](crate::core::UnifiedType) may not be used in your
/// application. [`Aurum`](crate) uses the [`Case`](crate::core::Case) trait to enforce this restriction.
/// The end users must define a single [`UnifiedType`](crate::core::UnifiedType) for their
/// application. DO NOT communicate between two [`Node`](crate::core::Node) instances with different
/// types, things are bound to go wrong.
///
/// [`Case::VARIANT`](crate::core::Case::VARIANT) can be used to create instances of its
/// [`UnifiedType`](crate::core::UnifiedType) from type information without needing to access the
/// variants of that [`UnifiedType`](crate::core::UnifiedType), which are defined in the macro. This
/// is how [`ActorRef`](crate::core::ActorRef) and [`Destination`](crate::core::Destination)
/// construct [`UnifiedType`](crate::core::UnifiedType) instances for forging.
///
/// If you are writing a library built on top of
/// [`Aurum`](crate), you can use [`Case`](crate::core::Case) to restrict your user's
/// [`UnifiedType`](crate::core::UnifiedType) to make sure it is compatible with your messages.
/// Users are required to list your dependent message types in their invocation of
/// [`unify`](crate::unify). This is how [`cluster`](crate::cluster) would normally be implemented,
/// but the [`Case`](crate::core::Case) bounds for [`UnifiedType`](crate::core::UnifiedType) include
/// the dependent message types for [`cluster`](crate::cluster) for the sake on convenience.
///
/// ```
/// use async_trait::async_trait;
/// use aurum_actors::{unify, AurumInterface};
/// use aurum_actors::core::{Actor, ActorContext, Case, UnifiedType};
/// use im;
/// use serde::{Serialize, Deserialize};
///
/// #[derive(AurumInterface, Serialize, Deserialize)]
/// enum MsgTypeForSomeThirdPartyLibrary {
///   #[aurum]
///   Something(InterfaceForSomeThirdPartyLibrary)
/// }
///
/// #[derive(Serialize, Deserialize)]
/// struct InterfaceForSomeThirdPartyLibrary;
///
/// struct LibraryActor;
/// #[async_trait]
/// impl<U> Actor<U, MsgTypeForSomeThirdPartyLibrary> for LibraryActor
/// where
///   U: UnifiedType
///     + Case<MsgTypeForSomeThirdPartyLibrary>
///     + Case<InterfaceForSomeThirdPartyLibrary>
/// {
///   async fn recv(
///     &mut self,
///     ctx: &ActorContext<U, MsgTypeForSomeThirdPartyLibrary>,
///     msg: MsgTypeForSomeThirdPartyLibrary
///   ) {
///     // logic
///   }
/// }
///
/// #[derive(AurumInterface)]
/// #[aurum(local)]
/// enum MyMsgType {
///   First,
///   Second
/// }
///
/// #[derive(AurumInterface)]
/// #[aurum(local)]
/// enum MyOtherMsgType {
///   Nonserializable(::std::fs::File),
///   #[aurum]
///   Serializable(InterfaceForSomeThirdPartyLibrary)
/// }
///
/// unify! { 
///   unified_name = pub MyUnifiedType;
///   root_types = {
///     MyMsgType,
///     MyOtherMsgType,
///     MsgTypeForSomeThirdPartyLibrary
///   };
///   interfaces = {
///     String,
///     InterfaceForSomeThirdPartyLibrary
///   };
/// }
/// ```
///
/// The syntax for [`unify`](crate::unify) is structured as an unordered set of key-value pairs
/// terminated by a semicolon. No key should be defined more than once. The keys are:
/// - `unified_name`: Required. Give an optional visibility and an identifier to name the
/// [`UnifiedType`](crate::core::UnifiedType).
/// - `root_types`: Optional. Contained in braces, it is an unordered, comma-terminated set of types
/// that implement [`RootMessage`](crate::core::RootMessage).
/// - `interfaces`: Optional. Contained in braces, it is an unordered, comma-terminated set of types
/// which are used as interfaces to root types. These are types annotated with `#[aurum]` on
/// invocations of [`AurumInterface`](crate::AurumInterface).
pub use aurum_actors_macros::unify;

/// Implements [`RootMessage`](crate::core::RootMessage) and other traits for a root message type.
///
/// This derive macro is applicable to enums only. It generates all the necessary implementations
/// for the message type to interact with the [`UnifiedType`](crate::core::UnifiedType) and produce
/// interfaces. It uses the `#[aurum]` atrribute to configure interfaces. The `#[aurum]` attribute
/// has a single optional argument: `local`. Use `#[aurum(local)]` to notify [`AurumInterface`] that
/// the interface is exclusively local.
///
/// ```
/// use aurum_actors::AurumInterface;
///
/// type StrStaticRef = &'static str;
///
/// #[derive(AurumInterface)]
/// #[aurum(local)]
/// enum DiverseMsgType {
///   #[aurum]
///   MyStringInterface(String),
///   #[aurum(local)]
///   MyNonserializableInterface(StrStaticRef),
///   MyOtherMsg(u128),
/// }
/// ```
///
/// In this example, because one of the message options is not serializable, the message type as a
/// whole is not serializable. However, you can use an
/// [`ActorRef<MyUnifiedType, String>`](crate::core::ActorRef) to send a string from a remote
/// machine to whatever actor uses this message type. You can also create a
/// [`LocalRef<&’static str>`](crate::core::LocalRef), but not a usable
/// [`ActorRef`](crate::core::ActorRef).
///
/// [`AurumInterface`] creates an implementation of [`RootMessage`](crate::core::RootMessage) for
/// the type it is invoked on. The [`RootMessage`](crate::core::RootMessage) implementation is
/// blanketed over all types implementing [`UnifiedType`](crate::core::UnifiedType). The blanket is
/// bounded by [`Case`](crate::core::Case) implementation for the root type and each remote
/// interface. In the example, the implementation is bounded by
/// [`Case<MyMsgType>`](crate::core::Case) and [`Case<String>`](crate::core::Case) but not
/// [`Case<&'static str>`](crate::core::Case).
///
/// [`AurumInterface`] implements [`From`](std::convert::From) on every interface type, local or
/// not. In the example, [`From<String>`](std::convert::From) and
/// [`From<&'static str>`](std::convert::From) are implemented for `MyMsgType`.
///
/// This macro's parsing is a work in progress, so for now you will need to create aliases for some
/// types.
pub use aurum_actors_macros::AurumInterface;
