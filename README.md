Make sure you have [cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html) installed. To build the docs, use this command from the project root:
```
$ cargo doc
```

The docs are generated HTML. The root doc is located here after generation:

```
./target/doc/aurum/index.html
```

Project layout:
```
src
├── bin : Contains drivers for experiments, including the benchmark
├── cluster : The cluster module and its submodules
│   ├── crdt : Delta-state CRDT dispersal
│   └── devices : External node tracking
├── core : The main foundation of the library
├── lib.rs
├── macros : A separate crate holding the AurumInterface and unify! procedural macros
└── testkit : Tools for failure injection
tests : Integration tests for Aurum
```
