# `actor_module!` migration

`actor_module!` now declares one actor directly. The old `manifest` keyword and
`actors: [...];` list are removed. Declare each actor with its public actor name:

```rust,ignore
actor_module! {
    pub actor logger: Logger {
        mailbox_capacity: 8;
        commands {}
        events { TotalChanged => log; }
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

For an old multi-actor list, create one `actor_module!` invocation per entry,
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
    commands { Add => add; }
    events { TotalChanged => []; }
};
```

This registers a typed no-op `publish(TotalChanged)` route. Duplicate command
routes still fail at compile time; event routes still fan out across composed
actors. Bounded capacities and backpressure behavior are unchanged.
