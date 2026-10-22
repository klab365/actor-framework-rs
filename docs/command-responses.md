# Typed command responses

Commands declare their handler output alongside the message type:

```rust,ignore
#[derive(actor_framework::ActorCommand)]
#[actor_command(response = Result<u32, CounterError>)]
struct GetCount;
```

The attribute value is parsed as a Rust type (not a string) and generates the
`Command` trait implementation with its associated `Response`. A route remains
`commands { GetCount => Counter::get_count; }`; the handler's async output
must match the declared type. The handler is an async inherent method with
`&mut self` and `#[actor_handler]`.

Tokio systems support `request(command).await`. A `Result<T, E>` handler gives
`Result<Result<T, E>, RequestError>` to its caller, separating handler rejection
from closed mailbox or lost response sender. `send` and `try_send` remain
fire-and-forget acceptance operations. If a handler returns a Result without a
requester, successful values are discarded and an error stops that actor by
panicking. A requester that is dropped does not cancel handling.

Existing command messages routed through a Tokio actor system must implement
`Command`; derive it with `#[actor_command(response = ())]` for existing unit
handlers. The shared trait has no executor or allocator dependency. Request/reply
transport is currently generated for Tokio; other adapters continue to provide
send/post only.
