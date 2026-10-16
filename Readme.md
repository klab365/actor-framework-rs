# actor-framework-rs

A Rust actor framework exploring statically composed actors for embedded and hosted targets. The publishable Cargo package is `actor-framework-rs`; its Rust library name remains `actor_framework`.

The implementation includes Tokio and [WebAssembly](docs/wasm.md) prototypes. The core is heap-free and `no_std`; both hosted adapters allocate. Heap-free Embassy integration remains planned.

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
- `examples/wasm/` — WASM `actor_module!` / `actor_system!` example; [run instructions](docs/wasm.md)
- `examples/module-lib/` and `examples/module-app/` — cross-crate module composition example

## Current status

The Tokio prototype supports reusable `actor_module!` manifests, bounded mailboxes, typed command routing, event fan-out, and coordinated `shutdown().await`. The WASM prototype adds typed bounded routes and local JS tasks. Embassy and request/response are not implemented yet. See [actor use cases](docs/actors-use-cases.md), [architecture notes](docs/architecture.md), the [Tokio design](docs/tokio-first-design.md), and [release instructions](docs/release.md).
