# actor-framework-rs

`actor-framework-rs` is a statically routed async actor framework. Actors own their
state, handle one message at a time, and communicate through typed, bounded
mailboxes. The published package is `actor-framework-rs`; import it as
`actor_framework`.

## Status and supported targets

This is an early-stage `0.1` release. The runtime-neutral core is `no_std`,
allocator-free, and does not select an executor. Generated actor systems are
available for:

- **Tokio** (the default feature) for `std` applications;
- **`wasm32-unknown-unknown`** with `wasm-bindgen` and a browser, worker, or
  Node host (`wasm` feature); and
- **Embassy** (`embassy` feature), using statically allocated task and mailbox
  storage.

Runtime lifecycle and storage semantics differ; consult [Tokio](docs/tokio-first-design.md),
[WASM](docs/wasm.md), and [Embassy](docs/embassy.md) before choosing a backend.
There is no dynamic actor registry, supervision/restart, delivery guarantee
beyond mailbox acceptance, or implicit timer API.

### Choose a backend deliberately

Use the default **Tokio** feature for a hosted application. Use only `wasm` for
`wasm32-unknown-unknown`; it uses local tasks and permits non-`Send` state. Use
only `embassy` for an Embassy application with static storage and an executor
`Spawner`. Do not treat the runtime-neutral `core` module as a ready-to-spawn
actor system: it supplies contracts, inline storage, and the caller-driven
`run` loop, while the runtime feature supplies generated systems and channels.

## Install

Tokio is enabled by default:

```toml
[dependencies]
actor-framework-rs = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

For a runtime-specific build, disable defaults explicitly:

```toml
# Browser/worker WASM
actor-framework-rs = { version = "0.1", default-features = false, features = ["wasm"] }

# Embassy / no_std application
actor-framework-rs = { version = "0.1", default-features = false, features = ["embassy"] }
```

`default-features = false` without a runtime feature exposes the heap-free
`core` contracts and inline `Mailbox`, not the `actor!`/`actor_system!` macros.

## Minimal Tokio system

Commands derive `ActorCommand` and declare their response type. An `actor!`
declaration describes exactly one actor; `actor_system!` composes declarations.
Handlers are async inherent methods. Typed `ActorAddress` parameters are
injected by `#[actor_handler]` and allow only the declared command/event route.

```rust
use actor_framework::{actor, actor_handler, actor_system};

#[derive(actor_framework::ActorCommand)]
#[actor_command(response = ())]
struct Add(u32);

struct Counter(u32);

impl Counter {
    #[actor_handler]
    async fn add(&mut self, message: Add) {
        self.0 += message.0;
    }
}

actor! {
    pub actor counter: Counter {
        mailbox_capacity: 8;
        commands { Add => Counter::add; }
        events {}
    };
}

actor_system! {
    pub struct App { actors: [counter]; }
}

#[tokio::main]
async fn main() {
    let app = App::builder().counter(Counter(0)).start().unwrap();
    app.send(Add(1)).await.unwrap();
    app.shutdown().await.unwrap();
}
```

See [`examples/tokio`](examples/tokio), the cross-crate composition example in
[`examples/module-lib`](examples/module-lib), and the runnable
[`examples/wasm`](examples/wasm). The Micro:bit example documents Embassy
setup in [`examples/embassy-microbit-v2`](examples/embassy-microbit-v2).

## Rules for integrations

These rules are the intended public API usage; do not infer alternate routing
or lifecycle APIs:

1. One `actor!` invocation declares one actor. Compose declarations only with
   `actor_system!`; provide every state value through its generated builder.
2. A routed handler is an async inherent method with `&mut self`, optional typed
   route addresses, and exactly one final message parameter. Mark every routed
   handler with `#[actor_handler]`. Free functions, trait methods, and dynamic
   actor lookup are not supported.
3. A command must derive `ActorCommand`, declare `#[actor_command(response =
   Type)]`, and be routed to exactly one handler. `system.request(command).await`
   returns that response or `RequestError`; `send` is fire-and-forget acceptance.
4. An event must be `Clone` when it has subscribers. Declare subscribers in each
   actor's `events` block, or write `Event => []` for an intentional no-op route.
5. Use a positive `mailbox_capacity` for generated actors. Capacity is a queue
   bound, not a processing-completion guarantee. The standalone core
   `Mailbox<M, 0>` is valid but permanently full and is not a rendezvous channel.
6. Handle `SendError` and `TrySendError`; do not assume retries, ordering across
   routes, atomic fan-out, persistence, or exactly-once processing.

## Capacity, backpressure, and failure

Every generated actor declares `mailbox_capacity`; it bounds queued messages,
not the message currently being handled.

- `send(...).await` and `ActorAddress::post(...).await` wait for capacity.
  Success means the mailbox accepted the message, **not** that handling finished.
- `try_send`/`try_post` never wait and return the original message in
  `TrySendError::Full` or `TrySendError::Closed`.
- Events are delivered sequentially to subscribers. A slow subscriber blocks
  publication; failure or cancellation can leave an already-delivered prefix.
  An explicitly declared `Event => []` route is a successful no-op.
- Closure, shutdown, and task failures reject later sends. Do not use mailbox
  acceptance as a persistence, processing, or exactly-once acknowledgement.

The precise shutdown behavior is backend-specific, especially for queued
messages. Design application-level acknowledgements where completion matters.

## Further documentation

- [Architecture and `no_std` core](docs/architecture.md)
- [Current actor declaration syntax](docs/actor-module-migration.md)
- [Typed actor addresses](docs/typed-actor-addresses.md)
- [Command responses](docs/command-responses.md)
- [When actors are useful](docs/actors-use-cases.md)
- [Release procedure](docs/release.md)
