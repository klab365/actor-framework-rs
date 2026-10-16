# Embassy adapter prototype (issue #6)

Use `default-features = false, features = ["embassy"]` and depend on
`embassy-executor` with the target-appropriate executor features. Runtime
features are intentionally composable for workspace `--all-features` checks;
when `tokio` is also enabled, Tokio owns `actor_system!`. An Embassy application
must therefore disable default features as shown. The shared core
remains runtime-neutral, allocator-free and `no_std`. The adapter and generated
system use `embassy-sync` channels; message/state types must meet the selected
mutex and executor's target bounds. No framework unsafe code is used.

## `actor_system!` integration

The Embassy backend is selected through the framework's internal macro re-export,
following the WASM backend pattern. Existing `actor_module!` manifests feed
`actor_system!`; the builder starts the actors:

```rust
let actors = AppActors::builder()
    .counter(Counter { value: 0 })
    .start(spawner)?;
```

The macro creates a statically allocated Embassy channel per actor, a generated
`#[embassy_executor::task(pool_size = 1)]` task, and typed send/try-send/publish
routes. `Spawner` is needed only at the system start call. Each actor has one task slot; the generated system additionally reserves one
private shutdown-coordinator slot. Mailbox capacity comes from
`mailbox_capacity` in its manifest.
Static storage is a singleton per generated system: a second start returns
`StartError::AlreadyStarted`. `start` first reserves every fixed Embassy task
slot and only then spawns tasks, so `TaskCapacityExhausted` leaves no partially
running system. The failed start reservation is released and can be retried with
a new builder. Allocate one slot per actor, one shutdown-coordinator slot, and
the app main task.

## Capacity and behavior

Mailbox `N` bounds queued messages only, excluding a message currently handled
and messages retained by blocked send futures. `try_send` is immediate and
returns the original message on Full. Async `send` waits for capacity; success
means accepted, not handled. Cancelling a pending send drops its message;
receiver cancellation does not dequeue. Event publish clones and sends
sequentially in manifest order; slow subscribers backpressure the publisher and
failure/cancellation may leave a delivered prefix. No rollback or fairness
promise is provided. Cyclic sends to full mailboxes can deadlock.

Generated systems provide cancellation-safe `shutdown().await`. It atomically
rejects new route sends and wakes a private, statically reserved coordinator;
that task appends one shutdown sentinel per actor mailbox, drains every message
accepted before that sentinel, and lets cancelled callers retry waiting.
Concurrent shutdown callers serialize while waiting. Pending route sends race the
close notification; if close wins, the unsent message is dropped and
`SendError` returned. Do not call shutdown from one of its own handlers: a
handler waiting for its own task would deadlock.
A panic or permanently blocked handler prevents completion; there is no panic
recovery or restart. The lower-level `embassy::Mailbox` remains an independent,
explicitly closable SPSC adapter for code using `core::run`; its semantics differ
from generated actor-system channels.

`examples/embassy` is a compile-check actor system targeting Cortex-M4F
(`thumbv7em-none-eabihf`). `examples/embassy-microbit-v2` is a flashable BBC
micro:bit v2/nRF52833 application: Button A sends a command, a Counter actor
publishes an event, and an LED actor flashes one matrix pixel. It uses only
`actor_module!`, `actor_system!`, and `.start(spawner)`. Install the target and
run `mise run check-embassy`, `mise run check-embassy-microbit-v2`, and
`mise run test-embassy`; CI compiles both targets. Flashing requires
`probe-rs-tools` and `cargo run --release` from the micro:bit example directory.
