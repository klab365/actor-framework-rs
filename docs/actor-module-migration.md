# `actor_module!` migration

Issue #17 removes the top-level `context { ... }` and `events: [...];` blocks.

Outgoing routes remain declared by typed `#[actor_handler]` parameters:
`ActorAddress<ActorCommand<M>>` permits `post`/`try_post`, and
`ActorAddress<ActorEvent<E>>` permits `publish`. The generated adapter still
requires the composed system to provide the corresponding typed route.

Declare event discovery beside its subscriber route:

```rust,ignore
actor_module! {
    pub manifest application;
    actors: [
        logger: Logger {
            mailbox_capacity: 8;
            commands {}
            events { TotalChanged => log; }
        }
    ];
}
```

For an event deliberately allowed without subscribers, use an explicit empty
handler list:

```rust,ignore
counter: Counter {
    mailbox_capacity: 8;
    commands { Add => add; }
    events { TotalChanged => []; }
}
```

This registers a typed `publish(TotalChanged)` route that succeeds as a no-op.
It creates no actor mailbox message or handler. The event registration can live
in any actor in the module; it does not mean that actor publishes the event.
