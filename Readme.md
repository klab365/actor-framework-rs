# actor-framework-rs

A Rust actor framework designed to run without heap allocation across embedded and hosted targets.

The project is at its initial design stage. The core crate is `no_std` by default and has no runtime dependencies. Runtime adapters for Tokio, WebAssembly, and Embassy are planned; their APIs and dependencies are not implemented yet.

## Design goals

- Heap-free core and bounded resource use
- Runtime-neutral actor and message APIs
- Support for `no_std` embedded targets and hosted environments
- Explicit mailbox capacity and overload behavior
- No unsafe code in the framework

## Project structure

- `src/actor-framework/` — runtime-neutral core crate
- `docs/` — use cases and architecture notes
- `examples/` — planned runtime-specific examples

## Current status

The Tokio prototype supports reusable `actor_module!` manifests, bounded mailboxes, typed command routing, event fan-out, and coordinated `shutdown().await`. Embassy/WASM adapters and request/response are not implemented yet. See [actor use cases](docs/actors-use-cases.md), [architecture notes](docs/architecture.md), and the [Tokio design](docs/tokio-first-design.md).
