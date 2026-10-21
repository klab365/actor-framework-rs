# Changelog

All notable changes to this project are documented in this file.

## [Unreleased]

### Added

- Typed command response contracts via `#[derive(ActorCommand)]` and Tokio request/reply routing. See [command response migration notes](docs/command-responses.md).

- Initial Tokio actor-system prototype with static module composition, typed command routing, event fan-out, bounded mailboxes, lifecycle shutdown, and reusable cross-crate actor modules.
