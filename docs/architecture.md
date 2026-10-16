# Architecture (initial design)

## Goals

The shared core must compile as `no_std` and must not require heap allocation. Runtime adapters may integrate with Tokio, WebAssembly host facilities, or Embassy, but should not impose their scheduling model on actor logic.

## Initial crate boundary

`actor-framework` is a `no_std` crate with no dependencies. Runtime features are placeholders until the respective adapters have been designed and tested. Enabling a feature must not silently make the core depend on `std` or allocate.

## Design constraints to preserve

- Mailbox capacity and storage should be statically bounded for embedded use.
- Message passing and actor state ownership should be explicit.
- Backpressure and full-mailbox behavior must be part of the API contract.
- The core must not assume threads, a global allocator, or a particular executor.
- Async support must not require heap-allocated futures in the core.

## Decisions from initial design discussion

- **Mailbox:** Use bounded channels supplied by the runtime adapter. Capacity and full-mailbox behavior remain explicit. This keeps the core independent of a particular synchronization primitive, but means mailbox APIs may differ between runtimes; adapters should share the same conceptual contract.
- **Actor interface:** Actors use asynchronous message handlers.
- **Future storage:** `Pin` is required for polling futures that may be self-referential, but pinning by itself does not provide storage. The implementation must also ensure the future is stored inline or in caller-provided/static storage and never box it. Prototype this constraint before finalizing the public API.
- **Runtime adapter layout:** Keep adapters in this workspace initially, as separate crates where dependency and target isolation require it. The core stays `no_std` and dependency-free.
- **WASM:** No WASM-specific target or host integration requirements have been selected yet; defer the adapter design until a concrete use case is chosen.
- **Embassy:** No specific executor compatibility or synchronization primitives have been selected yet; defer adapter design pending a concrete target and prototype.

## Tokio reference design

The first concrete API design is documented in [Tokio / std first design](tokio-first-design.md). It establishes module-manifest-declared actors and routes, typed command dispatch via `actors.send(...)`, event fan-out via `actors.publish(...)`, and bounded Tokio mailboxes. Tokio validates the public model first; it is not the heap-free reference implementation.

## Next design validation

Build compile-tested Tokio prototypes for macro-generated actors, static dispatch routes, and bounded mailbox semantics. Afterwards, validate that the same public model can use non-allocating, pinned future storage with Embassy. Verify that the core remains `no_std`, and evaluate the WASM adapter independently before stabilizing their APIs. Heap-free means the framework itself does not require dynamic allocation; applications may still choose to use an allocator on platforms that provide one.
