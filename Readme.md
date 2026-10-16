# actor-framework-rs

A Rust actor framework exploring statically composed actors for embedded and hosted targets. The publishable Cargo package is `actor-framework-rs`; its Rust library name remains `actor_framework`.

The runtime-neutral core is `no_std` and heap-free. Optional Tokio, WebAssembly, and Embassy adapters provide runtime-specific actor-system backends; Embassy storage and task capacities are statically bounded.

## Design goals

- Heap-free core and bounded resource use
- Runtime-neutral actor and message APIs
- Support for `no_std` embedded targets and hosted environments
- Explicit mailbox capacity and overload behavior
- No unsafe code in the framework

## Project structure

- `src/actor-framework/` — `actor-framework-rs` library package
- `docs/` — use cases and architecture notes
- `examples/tokio/` — Tokio example
- `examples/module-lib/` and `examples/module-app/` — cross-crate module composition example

## Current status

The Tokio adapter supports reusable `actor_module!` manifests, bounded mailboxes, typed routing, and coordinated shutdown. WASM and Embassy backends are prototypes with runtime-specific lifecycle limits. See [Embassy notes](docs/embassy.md), [WASM notes](docs/wasm.md), [architecture](docs/architecture.md), and [Tokio design](docs/tokio-first-design.md).
