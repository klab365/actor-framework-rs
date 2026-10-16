# WebAssembly adapter prototype (issue #5)

Enable `actor-framework-rs` with `default-features = false, features = ["wasm"]`.
The adapter targets `wasm32-unknown-unknown` with a wasm-bindgen JavaScript host
(browser, worker, or Node), not WASI. It uses `wasm-bindgen-futures::spawn_local`,
not Tokio or threads. Actor state, context, and messages may be non-`Send` (e.g.
`Rc<Cell<_>>`); spawned inputs must be owned and `'static`.

## Architecture and routing

The adapter implements the existing `ActorSendRoute`, `ActorPublishRoute`, and
`Receiver` contracts. Low-level actors can use the shared `core::run` loop;
manifest actors use generated serial dispatch loops, as on Tokio.
`channel::<Message>(capacity)` returns a concrete `Sender<Message>` command route
and a single receiver. `Publisher<Message, N>` stores an array of subscriber
senders, so fan-out is statically typed and statically sized. Applications build
context structs containing these concrete routes and implement the core route
traits to forward permitted messages. There is no registry, type erasure, or
runtime message-type lookup. Unsupported message types fail type checking.

`actor_module!` and `actor_system!` support the same manifest and builder syntax
as Tokio. On `wasm32` with feature `wasm`, systems generate local tasks, bounded
WASM channels, typed send/try_send/publish routes, and module-context capabilities.
Native targets with `tokio` keep the Tokio backend even with all features enabled.
With only `wasm`, macro expansion requires a WASM target. Duplicate commands are
rejected by the shared validation; unknown or forbidden routes fail type checking.
No `Send` bound or direct application Tokio dependency is introduced.
`start()` returns `wasm::StartError` for missing state or zero capacity, naming the
actor; all validation/channel creation occurs before spawning any actor.

The WASM adapter uses an allocating `async-channel` bounded FIFO rather than
adding synchronization/wakers to the core's exclusively owned inline mailbox.
This reuses cancellation-safe channel wakeups and keeps executor dependencies
out of the heap-free core. Local tasks also allocate. The core still builds with
no dependencies, no allocator, and `no_std`; framework unsafe code is forbidden.
Dependencies are not claimed to be internally unsafe-free.

## Capacity, backpressure, and errors

- Capacity is explicit and excludes the currently executing message. Zero is
  rejected with `InvalidCapacity`; there is no rendezvous or overflow eviction.
- `try_send` returns the original message on `Full` or `Closed` (closed wins).
- `send_route().await` waits for capacity and returns `SendError` on closure.
  Acceptance is not completion. Cancelling a pending send drops its message
  without enqueuing it. Waiting producers retain messages: bounded queue size
  is not a bound on total application memory or task count. Fairness is not
  promised. Cyclic/self-sends can deadlock when mailboxes fill.
- Publishing clones sequentially in array order. Zero subscribers succeeds;
  closure stops delivery. Failure/cancellation can leave a delivered prefix,
  without rollback. A slow subscriber backpressures the publisher.
- Closing a sender explicitly closes the entire shared queue, wakes waiters,
  rejects sends, and drains accepted messages. Dropping the last sender does
  the same. Context-held sender clones count, so reference cycles require an
  explicit close. Dropping the receiver rejects sends and drops queued messages.
- Receive cancellation does not remove messages. `None` means closed and drained.
- Low-level `spawn_actor` tasks are detached. Macro systems provide
  `shutdown().await`: close every mailbox, drain accepted messages, wait for every
  task. Unlike Tokio, queued messages are not discarded. Outgoing sends from
  draining handlers can fail because all destinations are already closed; handle
  those errors. Use an application acknowledgement before shutdown if downstream
  delivery must complete (as demonstrated in the example).
- Dropping a macro system closes all mailboxes without waiting. Shutdown is
  retryable after cancellation and idempotent after completion; concurrent calls
  return `ShutdownInProgress`. It does not abort a stuck handler. Neither backend
  operation supplies WASM panic recovery/restart: a task panic follows host policy
  and can prevent shutdown completion. Long CPU handlers must yield.
  Browser/worker execution is intended but only Node execution is tested here.

## Executable test and checks

`examples/wasm/` is a standalone WASM cdylib using `actor_module!`,
`actor_system!`, a builder, commands, events, non-`Send` actor state and an explicit
completion acknowledgement. `run_example()` returns a Promise. Run with Node:

```sh
mise run check-wasm-example
wasm-bindgen --target nodejs --out-dir /tmp/actor-wasm-example \
  examples/wasm/target/wasm32-unknown-unknown/debug/actor_framework_wasm_example.wasm
node -e 'require("/tmp/actor-wasm-example/actor_framework_wasm_example.js").run_example().catch(e => { console.error(e); process.exitCode = 1; })'
```

For a browser, use `wasm-bindgen --target web`, serve its output, import the JS
module, await its default initializer, then `await run_example()`.
`tests/wasm_macros.rs` runs generated routes, fan-out, builder failures,
backpressure and shutdown under Node. `tests/wasm.rs` covers the low-level path;
native tests cover full/closed errors, cancellation and partial fan-out.

```sh
mise run test-wasm
rustup target add wasm32-unknown-unknown
mise run check-wasm
# Install the wasm-bindgen-cli version matching wasm-bindgen in Cargo.lock.
cargo install wasm-bindgen-cli --version 0.2.129 --locked
# Requires Node on PATH; wasm-bindgen-test defaults to Node.
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
  cargo test -p actor-framework-rs --no-default-features --features wasm \
  --target wasm32-unknown-unknown --test wasm --test wasm_macros
```

CI compiles the adapter and executable test for WASM, runs native semantics,
and runs the JS test under Node with a matching runner from the lockfile.
