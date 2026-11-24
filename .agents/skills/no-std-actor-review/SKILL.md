---
name: no-std-actor-review
description: Review changes to the actor framework's shared core for no_std, heap-free, runtime-neutral compatibility. Use when modifying core APIs, dependencies, feature flags, allocation, mailbox capacity, or executor integrations.
---

# Review shared-core compatibility

Use this checklist when changing the shared actor framework core or its feature boundaries.

1. Read the repository `AGENTS.md` and inspect the changed code and feature/dependency declarations.
2. Confirm the shared core remains `no_std` and does not require a global allocator or a particular executor.
3. Check that runtime-specific dependencies and APIs remain behind their intended features. Do not add a runtime dependency to the core without explicit design discussion.
4. Verify bounded storage, capacity limits, backpressure, and failure behavior are explicit and preserved.
5. Check that unsafe code has not been introduced.
6. Identify affected integrations (Tokio, WebAssembly, Embassy) and run the narrowest relevant checks. For broader changes, run the repository's `mise run check`, `mise run check-format`, `mise run lint`, `mise run test`, and `mise run docs` checks.
7. Report concrete findings with file and line references. Distinguish confirmed regressions from risks or missing test coverage; do not claim a check passed unless it was run.

Do not recommend heap-backed collections or executor-specific abstractions in the shared core. If an integration needs them, keep them in the integration layer and preserve the core's bounded, runtime-neutral contract.
