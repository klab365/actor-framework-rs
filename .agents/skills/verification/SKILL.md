---
name: verification
description: Verify code changes with focused checks and report evidence accurately. Use after implementation or when diagnosing a CI failure.
---

# Verify repository changes

1. Read the changed files and identify the behavior and project boundary affected.
2. Run the narrowest relevant test or check first. For Rust changes, use the repository's mise tasks where applicable.
3. For broad changes, run the repository baseline: `mise run check`, `mise run check-format`, `mise run lint`, `mise run test`, and `mise run docs`.
4. For CI failures, inspect the failed job log and distinguish code failures from environment, toolchain, or snapshot failures. Reproduce the failing command locally when practical.
5. Do not treat a command that was not run, incomplete output, or an assumed CI result as a pass.
6. Report each command and its result. For failures, include the relevant diagnostic and whether it is reproduced. Do not change unrelated files to silence checks.
