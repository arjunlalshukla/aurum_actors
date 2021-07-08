This crate is an implementation of the [actor model]. Code examples and explainations are in the [API documentation](https://docs.rs/aurum_actors).

## Features
- Actors are typed: they may only receive one type as their messages.
- Actor can have interfaces for subsets of its possible messages.
- Actors are distributed: they can be sent messages from another machine.
- Serialization is customizable and built-in.
- Actor references are forgeable, they can be created from scratch.
- This crate has excellent type safety and catches many potential bugs at compile time.
- The capability of actor systems to form clusters is included.
- Many kinds of [CRDT] can be shared among cluster members.
- You can define your own [CRDT], and share it in the cluster.
- Aurum's feature set is well-suited to IoT applications.

## How is Aurum different from other actor models?
- Most actor model implementations are exclusively local.
- Most distributed actor models are untyped, which decreases type-safety.
- Most actor model implementations do not allow you to forge actor references.
- Serialization in Aurum is more powerful and flexible.

## Current state of the project
Aurum is brand new, and we welcome new users to experiment with it. We are pre-1.0 at the moment,
and new versions may include significant breaking changes as we figure out the API.

[actor model]: https://en.wikipedia.org/wiki/Actor_model
[CRDT]: https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type