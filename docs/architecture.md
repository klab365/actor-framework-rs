# Runtime-neutral actor core

## Crate boundary (issue #4)

Keep one public crate, `actor-framework-rs`: its always-available `core` module
and root re-exports contain runtime-independent contracts and inline storage.
With `default-features = false` the crate is genuinely `no_std`, has no
dependencies, does not import `alloc`, and forbids unsafe code. The default
`tokio` feature enables the existing adapter and optional proc-macro dependency.
The macros currently generate Tokio code and are therefore only exported with
that feature. `wasm` and `embassy` remain placeholders, not adapters.

A separate core crate is unnecessary until an adapter needs independent
versioning. No Tokio types, `Send`, `'static`, boxed futures, or dynamic registry
appear in the shared contracts. Application message/state types may themselves
allocate; the framework does not require them to do so.

## Actor and message contracts

Messages are ordinary owned types. `Actor<M, C>::handle` borrows exclusive state
and a context and returns a concrete async future. `run` consumes messages from
a `Receiver` serially and awaits each handler to completion. Both support local,
borrowed, non-`Send` state and contexts. No marker derives are needed.

`ActorSendRoute`, `ActorPublishRoute`, `ActorContext`, `SendError`, and
`TrySendError` are extracted from the original Tokio adapter without changing
root paths or signatures. Generated Tokio handlers continue to use free async
functions and generated message enums rather than being forced through a new
`Actor` wrapper. These are two statically dispatched entry points, not a dynamic
actor abstraction. Manifests still define command uniqueness and event fan-out.

## Storage and backpressure

`Mailbox<M, N>` owns `[Option<M>; N]` inline. It is const-initializable and needs
neither allocation nor unsafe initialization. `try_send` returns the original
message on `Full` or `Closed`; closed takes precedence. FIFO dequeue frees a
slot; capacity excludes the message currently handled. Zero capacity is valid,
always full, and is not a rendezvous channel. Closing rejects new sends but
allows draining; dropping storage drops all remaining messages.

This storage is deliberately **not a concurrent channel**. Its exclusive-access
operations do not wait or register wakers. A runtime adapter must provide safe
ownership/synchronization and wakeups around its chosen bounded storage before
implementing `Receiver`. A pending receive must be cancellation-safe; `None`
means permanently closed and drained. The core does not implement a misleading
async receive on an unsynchronized queue.

Routes wait for bounded capacity, fail on closure, and acknowledge acceptance,
not completion. Publishing to zero subscribers succeeds. Fan-out need not be
atomic: adapters must document order and partial-delivery/cancellation behavior.
See [Tokio design](tokio-first-design.md) for its existing sequential fan-out,
shutdown, task failure, and backpressure semantics.

## Executor integration

`run(&mut actor, &context, &mut receiver)` is the executor boundary, not a spawn
API. Its future borrows caller-owned storage and is stored and pinned inline
(e.g. `core::pin::pin!`) or in statically reserved executor task storage. Pinning
alone does not allocate or provision storage. The executor owns polling and
wakeup registration; no executor trait with boxed futures or global registry is
needed. The compile-tested core test uses inline pinning and borrowed `Cell`
state, proving that neither `Send` nor `'static` is imposed.

`run` drains until closure. Cancelling/dropping it also cancels the current
handler and drops its in-flight message; pending queue storage remains owned by
the caller. It offers no supervision or panic recovery. Tokio retains its
separate generated loops: shutdown lets the active handler finish and discards
queued messages. This difference is explicit, not an implied shared lifecycle.

## Validation and deferred work

`mise run check` selects the package explicitly with `--no-default-features`:
workspace-wide checks can accidentally re-enable Tokio through example feature
unification. `mise run test-core` checks storage, ownership/drop behavior and
inline local execution without runtime features. Existing integration and UI
tests continue to validate Tokio, routing, backpressure and shutdown.

Embassy/static task integration and WASM host integration remain future adapter
work. No particular embedded target, synchronization primitive or interrupt
model has been selected. This core defines and tests their integration boundary;
it does not claim to implement those adapters. Heap-free executor storage must
still be validated against a concrete target before stabilizing that adapter.
