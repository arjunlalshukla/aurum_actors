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
