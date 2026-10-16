# Changelog

All notable changes to this project are documented in this file.

## [Unreleased]

## [0.1.1](https://github.com/klab365/actor-framework-rs/compare/actor-framework-rs-v0.1.0...actor-framework-rs-v0.1.1) - 2026-10-16

### Added

- complete actuator context ergonomics
- add ergonomic actor handlers
- *(embassy)* add statically bounded actor runtime adapter
- *(wasm)* add macro-based actor runtime adapter
- *(core)* add heap-free runtime-neutral actor contracts

### Other

- remove handler context
- generate application actuator context
- *(tokio)* cover partial publish and shutdown queues

### Added

- Initial Tokio actor-system prototype with static module composition, typed command routing, event fan-out, bounded mailboxes, lifecycle shutdown, and reusable cross-crate actor modules.
