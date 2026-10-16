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
mise run lint
mise run test
mise run docs
mise run publish-dry-run
```

`mise run publish-dry-run` packages and verifies publishability without uploading. Never run `cargo publish` manually as part of routine validation.
