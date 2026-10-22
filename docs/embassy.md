# Embassy backend

Enable the heap-free Embassy backend with:

```toml
actor-framework-rs = { version = "0.1", default-features = false, features = ["embassy"] }
```

Generated systems use statically allocated Embassy task, mailbox, lifecycle,
and command-reply storage. They are started with the generated builder and an
`embassy_executor::Spawner`; see the flashable
[Micro:bit v2 example](../examples/embassy-microbit-v2/README.md). This backend
is intended for Embassy executors and does not require `std` or a global
allocator.

## Storage and routing

Declare actors with the same one-actor `actor!` and composition
`actor_system!` syntax as the Tokio example. Each mailbox capacity must be
nonzero for generated Embassy systems. Startup can fail when a required state
is missing, static mailbox storage was already initialized, or no fixed task
slot is available. Static storage means a generated system is single-start for
that storage instance.

Mailboxes are bounded and heap-free. `send`/`post` wait for capacity;
`try_send`/`try_post` return the original message when full or closed. Success
only acknowledges mailbox acceptance. Event fan-out is sequential, can
backpressure on a subscriber, and is not atomic: a failure can leave a delivered
prefix. A handler should handle route errors when downstream delivery matters.

## Requests and shutdown

Commands derived with `ActorCommand` may be called with `request(command).await`.
The handler response is carried through a statically allocated reply-slot pool;
set its per-handler capacity with `request_capacity`:

```rust,ignore
impl State {
    #[actor_framework::actor_handler(request_capacity = 4)]
    async fn query(&mut self, command: Query) -> Result<Value, QueryError> {
        // ...
    }
}
```

Additional callers wait for a reply slot. A dropped requester does not cancel
handling; its slot is reclaimed when the actor replies or drops the queued
envelope. If the reply sender is dropped, the caller receives
`RequestError::ResponseDropped`.

`shutdown().await` closes routes and waits for generated actor tasks. Accepted
messages are drained; sends issued by draining handlers can fail because all
routes have closed. It does not abort a stuck handler and provides neither panic
recovery nor supervision. Use an application acknowledgement before shutdown
when downstream processing must complete.
