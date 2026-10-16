# Repository guidance

## Project overview

`actor-framework-rs` is an early-stage Rust actor framework intended for heap-free `no_std` use as well as Tokio, WebAssembly, and Embassy integrations. The shared core must not require a global allocator or a particular executor.

## Required checks

Before finishing a change, run the narrow relevant checks. The baseline checks are:

```sh
mise run check
mise run check-format
mise run lint
mise run test
mise run docs
```

## Coding guidelines

- Keep `unsafe` forbidden.
- Preserve `no_std` and heap-free support in the core.
- Do not introduce runtime dependencies into the core without explicit design discussion.
- Make bounded capacity, backpressure, and failure behavior explicit in public APIs.
- Document public APIs and add tests for behavior changes.
