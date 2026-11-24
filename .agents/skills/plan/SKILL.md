---
name: plan
description: Create a focused implementation plan for a feature, bug fix, or technical investigation in this repository.
---

# Plan repository work

1. Read `AGENTS.md` and inspect the relevant source, tests, configuration, and documentation before proposing changes.
2. Restate the goal and acceptance criteria. If a missing decision would materially change the approach, ask one focused question rather than planning around an assumption.
3. Break the work into the smallest ordered, verifiable steps. Name the files or components likely to change and note dependencies between steps.
4. Include tests and documentation where behavior or public APIs change. Account for the project's no_std, heap-free core and runtime boundaries.
5. Identify the narrowest relevant checks, then the repository baseline checks if the change is broad.
6. Keep the plan proportional: avoid speculative refactors, unrelated cleanup, and steps that merely repeat the request.

A plan is a proposal, not permission to modify files. Do not edit code while planning unless explicitly asked to prototype or implement.
