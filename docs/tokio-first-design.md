# Tokio backend

The default `tokio` feature provides generated, statically routed actor systems
for `std` applications. It uses Tokio bounded `mpsc` mailboxes and spawned
actor tasks; it is not the heap-free execution path. The shared contracts remain
available without Tokio; see [architecture](architecture.md).

## Declaration and composition

A message sent as a command implements `Command`, normally via
`#[derive(ActorCommand)]`. Its `response` is also the return type required from
its handler. Events are ordinary `Clone` values because each subscriber gets a
copy.

```rust
#[derive(actor_framework::ActorCommand)]
#[actor_command(response = ())]
struct Add(u32);

#[derive(Clone)]
struct TotalChanged(u32);

struct Counter(u32);
struct Logger;

impl Counter {
    #[actor_framework::actor_handler]
    async fn add(
        &mut self,
        events: ActorAddress<ActorEvent<TotalChanged>>,
        message: Add,
    ) {
        self.0 += message.0;
        events.publish(TotalChanged(self.0)).await.expect("logger is running");
    }
}

impl Logger {
    #[actor_framework::actor_handler]
    async fn log(&mut self, event: TotalChanged) {
        println!("total: {}", event.0);
    }
}

actor_framework::actor! {
    pub actor counter: Counter {
        mailbox_capacity: 8;
        commands { Add => Counter::add; }
        events {}
    };
}

actor_framework::actor! {
    pub actor logger: Logger {
        mailbox_capacity: 8;
        commands {}
        events { TotalChanged => Logger::log; }
    };
}

actor_framework::actor_system! {
    pub struct App { actors: [counter, logger]; }
}
```

Supply every actor state through the generated builder, then call `start()`.
The resulting system exposes `send`, `try_send`, `publish` for declared routes,
`request` for commands, and `shutdown`. A command must have one handler;
duplicate command routes fail to compile. An event may have zero or more
subscribers. To expose an intentional no-subscriber event route, write
`events { Event => []; }`.

`#[actor_handler]` is required on each routed async inherent method, including
handlers without route contexts. A route context goes after `&mut self` and
before the one message parameter:

- `ActorAddress<ActorCommand<M>>` permits `post(M)` and `try_post(M)`;
- `ActorAddress<ActorEvent<E>>` permits `publish(E)`.

The generated router type is inferred; users do not construct an address.
See [typed actor addresses](typed-actor-addresses.md) and the runnable
[`examples/tokio`](../examples/tokio).

## Backpressure and lifecycle

A mailbox capacity counts queued messages and excludes the handler's current
message. `send` waits when a mailbox is full; `try_send` returns `Full(message)`
instead. Both return a closure error if the destination has stopped. Successful
send/publish means acceptance, not completed handling.

Publication clones and sends sequentially in actor declaration order. It waits
for each bounded mailbox. If a later subscriber is closed, or publication is
cancelled, already accepted deliveries remain and later ones are not rolled
back. A no-subscriber route succeeds immediately.

`shutdown().await` closes routes, lets active handlers finish, discards queued
messages, and waits for actor tasks. Future sends fail. Dropping a system starts
shutdown but cannot wait for tasks or report failures. A panic or unexpected
task exit is reported by `shutdown` as `ActorTaskError`. There is no restart or
supervision facility.

Tokio tasks require the `Send + 'static` bounds imposed by `tokio::spawn`.
Those bounds are backend-specific, not a property of the runtime-neutral core.
