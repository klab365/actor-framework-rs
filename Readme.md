# actor-framework-rs

`actor-framework-rs` is a statically routed async actor framework. Actors own their
state, handle one message at a time, and communicate through typed, bounded
mailboxes. The published package is `actor-framework-rs`; import it as
`actor_framework`.

## Status and supported targets

The `1.0` release provides a stable public API for the documented features. The
runtime-neutral core is `no_std`, allocator-free, and does not select an executor.
Generated actor systems are
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
actor-framework-rs = "1"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

For a runtime-specific build, disable defaults explicitly:

```toml
# Browser/worker WASM
actor-framework-rs = { version = "1", default-features = false, features = ["wasm"] }

# Embassy / no_std application
actor-framework-rs = { version = "1", default-features = false, features = ["embassy"] }
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

## Requests, responses, and reply slots

A command declares its response type. `request` waits for that typed response;
the outer `Result` is transport/lifecycle failure and an inner `Result`, when
used as the response type, is the handler's application result.

```rust,ignore
use actor_framework::{ActorAddress, ActorRequest};

#[derive(actor_framework::ActorCommand)]
#[actor_command(response = Result<u32, LookupError>)]
struct Lookup(Id);

impl Router {
    #[actor_framework::actor_handler]
    async fn route(
        &mut self,
        lookup: ActorAddress<ActorRequest<Lookup>>,
        id: Route,
    ) -> Result<Result<u32, LookupError>, actor_framework::RequestError> {
        lookup.request(Lookup(id.0)).await
    }
}
```

`ActorRequest<M>` is an injected, statically permitted capability: a handler
can request only command types declared by the composed system. It has no global
registry and works identically in Tokio, WASM, and Embassy. Its
`.request(message).await` returns `Result<M::Response, RequestError>`:

- `RequestError::Closed`: shutdown or task exit closed the destination before
  the request was accepted.
- `RequestError::ResponseDropped`: the request was accepted, but its reply
  endpoint was dropped (for example, the actor task failed).

Dropping the caller's request future stops waiting; it does **not** cancel an
accepted handler. Avoid request cycles (including requesting the same
single-threaded actor from its active handler), as they can deadlock.

### Choosing capacities

`mailbox_capacity` belongs in every `actor!` declaration and limits queued
commands/events for that actor. It does not include the handler currently
executing, and it does not limit how many callers are waiting for responses.
Choose it from the maximum burst of accepted work that the actor may buffer;
`send`, `post`, and accepted requests wait when it is full.

For Embassy, each requestable handler also has a statically allocated reply-slot
pool. Configure its number of concurrent in-flight requests with
`#[actor_handler(request_capacity = N)]` (default: `1`; `N` must be positive):

```rust
impl Worker {
    #[actor_framework::actor_handler(request_capacity = 4)]
    async fn lookup(&mut self, message: Lookup) -> Result<u32, LookupError> {
        // ...
    }
}

actor_framework::actor! {
    actor worker: Worker {
        mailbox_capacity: 8;
        commands { Lookup => Worker::lookup; }
        events {}
    };
}
```

Here up to four callers can reserve reply storage while requests are queued or
handled; further callers wait for a slot. Increasing `request_capacity` consumes
static Embassy storage proportional to the response type and creates no heap
allocation. Tokio and WASM retain their backend request machinery; the
attribute is accepted so one handler definition remains portable. Set both
capacities deliberately: `mailbox_capacity` bounds buffered work, while Embassy
`request_capacity` bounds simultaneous response slots.

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
- [Embassy reply-slot and shutdown semantics](docs/embassy.md)
- [When actors are useful](docs/actors-use-cases.md)
- [Release procedure](docs/release.md)
