# Typed actor route contexts

Handlers declare the routes they need using typed `ActorAddress` parameters:

```rust,ignore
use actor_framework::{ActorAddress, ActorCommand, ActorEvent, actor_handler};

#[actor_handler]
async fn count_press(
    store: ActorAddress<ActorCommand<StoreTotal>>,
    events: ActorAddress<ActorEvent<PressObserved>>,
    counter: &mut Counter,
    _message: ButtonPressed,
) {
    counter.total += 1;
    store.post(StoreTotal(counter.total)).await?;
    events.publish(PressObserved(counter.total)).await?;
}
```

`ActorCommand<M>` enables `post` and `try_post`; `ActorEvent<E>` enables
`publish`. Wrong operations and missing routes fail at compile time through the
generated route-trait bounds. Event messages must implement `Clone` for fan-out.
Publishing is sequential in manifest order: it backpressures on each bounded
mailbox, can partially deliver if a later send fails or the future is cancelled,
and succeeds when there are no subscribers. `post` success means accepted, not
handled. Calls after shutdown return `SendError`.

No registry, `TypeId`, serialization, heap allocation, or unsafe code is used.
The shared `ActorAddress` wrapper is runtime-neutral; Tokio and WASM bind the
same generated route traits to their bounded channels, and Embassy binds them to
statically allocated channels/task storage.

`#[actor_handler]` parses context parameters and emits a hidden generic adapter.
The selected `actor_system!` backend binds that adapter to its private generated
router type. This keeps reusable handler crates independent of application/system
types and requires no duplicate `outgoing` declaration. The `actor_module!`
manifest syntax places event discovery alongside actor event routes: use
`Event => handler` for subscribers or `Event => []` to explicitly retain a
no-subscriber publish route. See [the migration guide](actor-module-migration.md). Every manifest-referenced handler must carry
`#[actor_handler]`, including handlers that take no route contexts, because the
runtime backends invoke the generated adapter.

The [Micro:bit v2 example](../examples/embassy-microbit-v2/README.md) exercises
Embassy context injection. `examples/module-lib` and `examples/module-app`
exercise a Tokio context crossing a crate boundary. Existing Tokio integration
tests cover event fan-out to multiple subscribers.
