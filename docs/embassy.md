# Embassy command requests

Derived commands can be requested from generated Embassy actor systems with
`request(command).await`. The handler response is carried through a statically
allocated reply-slot pool, so the core remains heap-free. Pool capacity is set
on the handler:

```rust,ignore
impl State {
    #[actor_framework::actor_handler(request_capacity = 4)]
    async fn query(&mut self, command: Query) -> Result<Value, QueryError> {
        // ...
    }
}
```

The capacity bounds concurrent in-flight requests for that handler. Additional
callers wait for a free slot. Dropping a requester marks its slot abandoned;
the slot is reclaimed when the actor replies or drops the queued envelope, so a
late response cannot be mistaken for another request. If the actor task exits
while holding a request envelope, dropping the reply sender wakes its caller
with `RequestError::ResponseDropped`. Omitting the attribute defaults to one
concurrent request slot.
