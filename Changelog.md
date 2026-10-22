# Changelog

All notable changes to this project are documented in this file.

## [Unreleased]

## [1.0.0](https://github.com/klab365/actor-framework-rs/compare/actor-framework-rs-v0.1.0...actor-framework-rs-v1.0.0) - 2026-10-22

### Added

- Statically routed actor framework with bounded mailboxes and typed command/event routing across Tokio, WebAssembly, and Embassy.
- One actor per `actor!` declaration, composed in `actor_system!` with heap-free core support.
- Typed command responses and actor methods using `#[actor_handler]`.

### Changed

- Stabilize the public API for the first 1.0 release.
- Replace free-function actor handlers with inherent actor methods; see the migration guide.

### Removed

- The earlier `actor_module!` syntax and free-function handler API.

### Added

- Typed command response contracts via `#[derive(ActorCommand)]` and Tokio request/reply routing. See [command response migration notes](docs/command-responses.md).

- Initial Tokio actor-system prototype with static module composition, typed command routing, event fan-out, bounded mailboxes, lifecycle shutdown, and reusable cross-crate actor modules.
