# `actor!` migration

## Method handlers (issue #30)

Free-function `#[actor_handler]` handlers are no longer supported. Move
`async fn add(state: &mut Counter, message: Add)` into `impl Counter`, replace
`state` with `self`, and change `Add => add` to `Add => Counter::add`:

```rust,ignore
impl Counter {
    #[actor_handler]
    pub async fn add(&mut self, message: Add) {
        self.total += message.0;
    }
}
```

Put typed `ActorAddress` parameters after `&mut self`, before the message.
The attribute generates an associated async adapter and infers its router
bounds; it does not box futures or require an executor. Command response types
and unit-returning event handlers retain their existing contracts. Mailbox
capacity, backpressure, and Embassy `request_capacity` are unchanged.

Only async inherent methods with `&mut self` and exactly one message are
supported. Shared/owned receivers, associated functions, trait methods and
generic methods are unsupported. Trait methods are rejected by Rust's explicit
visibility diagnostics on the generated associated items; the attribute cannot
inspect the surrounding impl. Keep generics and bounds on the inherent impl:
`impl<T: ...> Counter<T>`. Use a concrete manifest state `Counter<u32>` and
method path `Counter::<u32>::add` (turbofish required). Generic actor manifests
and generic method arguments are not supported. Concrete generic types must
still satisfy the chosen backend's normal lifetime/Send/storage constraints.

Keep method visibility suitable for the composition site, especially for
cross-crate manifests (`pub` methods). Generated adapters and request-capacity
constants preserve that visibility. No nightly async features are needed.

## Single-actor declarations

`actor!` now declares one actor directly. The old `manifest` keyword and
`actors: [...];` list are removed. Declare each actor with its public actor name:

```rust,ignore
actor! {
    pub actor logger: Logger {
        mailbox_capacity: 8;
        commands {}
        events { TotalChanged => Logger::log; }
    };
}
```

Compose actor declarations by name in `actor_system!`:

```rust,ignore
actor_system! {
    pub struct App {
        actors: [counter, logger];
    }
}
```

For an old multi-actor list, create one `actor!` invocation per entry,
retaining each actor name, state type, mailbox capacity, and typed routes. The
actor names are used both in `actor_system!` and by builder setters such as
`.counter(...)` and `.logger(...)`. Across crates, export/import each actor
name (see `examples/module-lib` and `examples/module-app`).

Issue #17 also removes top-level `context { ... }` and `events: [...];` blocks.
Outgoing routes remain declared by typed `#[actor_handler]` parameters:
`ActorAddress<ActorCommand<M>>` permits `post`/`try_post`, and
`ActorAddress<ActorEvent<E>>` permits `publish`.

An event subscriber is declared beside its event route. For an event deliberately
allowed without subscribers, use `Event => []`:

```rust,ignore
pub actor counter: Counter {
    mailbox_capacity: 8;
    commands { Add => Counter::add; }
    events { TotalChanged => []; }
};
```

This registers a typed no-op `publish(TotalChanged)` route. Duplicate command
routes still fail at compile time; event routes still fan out across composed
actors. Bounded capacities and backpressure behavior are unchanged.
