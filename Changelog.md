# Changelog

All notable changes to this project are documented in this file.

## [Unreleased]

## [0.1.1](https://github.com/klab365/actor-framework-rs/compare/actor-framework-rs-v0.1.0...actor-framework-rs-v0.1.1) - 2026-10-22

### Added

- *(actor)* replace free-function handlers with methods
- *(macros)* declare one actor per module
- *(macros)* simplify actor module event routes
- add typed command responses
- add typed actor route contexts
- complete actuator context ergonomics
- add ergonomic actor handlers
- *(embassy)* add statically bounded actor runtime adapter
- *(wasm)* add macro-based actor runtime adapter
- *(core)* add heap-free runtime-neutral actor contracts

### Other

- prepare public documentation for first release
- *(macros)* rename actor_module to actor
- *(macros)* update empty actor declaration snapshot
- *(test)* format no-subscriber handler
- remove handler context
- generate application actuator context
- *(tokio)* cover partial publish and shutdown queues

### Added

- Typed command response contracts via `#[derive(ActorCommand)]` and Tokio request/reply routing. See [command response migration notes](docs/command-responses.md).

- Initial Tokio actor-system prototype with static module composition, typed command routing, event fan-out, bounded mailboxes, lifecycle shutdown, and reusable cross-crate actor modules.
