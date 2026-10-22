# Releases

The workspace publishes two crates together:

- `actor-framework-rs` (Rust library name `actor_framework`)
- `actor-framework-macros`

Release-plz opens a release PR on pushes to `main`. After that PR is reviewed and merged, the Release-plz workflow publishes the tagged workspace release. Versions are shared through `[workspace.package]` in the root `Cargo.toml`; the macros dependency version in `src/actor-framework/Cargo.toml` must match it.

## GitHub setup

Add a repository Actions secret named `CRATESIO_TOKEN` containing a crates.io API token with publish rights for both crates. The workflow uses the repository `GITHUB_TOKEN` for GitHub releases and PRs. Workflow permissions are scoped per job.

## Local checks

```sh
mise run check-format
mise run check
mise run check-tokio
mise run check-runtime-feature-matrix
mise run test-core
mise run test-embassy
mise run test-wasm
mise run check-wasm
mise run check-wasm-example
mise run lint
mise run test
mise run docs
mise run publish-dry-run
```

`mise run publish-dry-run` packages and verifies publishability without uploading. The WASM checks require the `wasm32-unknown-unknown` target; the executable WASM tests additionally require Node as configured in `mise.toml`. `mise run check-embassy-microbit-v2` cross-compiles the hardware example when its Cortex-M target is installed, but does not validate a physical device. Never run `cargo publish` manually as part of routine validation.
