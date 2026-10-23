# Actor task memory and execution resources

`mailbox_capacity` bounds **queued envelopes**, not task memory, stack depth,
concurrent requests, or total application RAM. There is deliberately no universal
`stack_size` actor option. An async task is a future polled by an executor, not
an independently stacked thread. State retained across `.await` contributes to
the future; synchronous calls and polling also consume the executor's stack.
Large locals can affect either or both, depending on compiler optimization.

## Backend ownership

| Backend | Generated execution | Where memory is controlled |
| --- | --- | --- |
| Tokio | One `tokio::spawn` per actor | Heap-backed task/future and channels; runtime builder controls worker-thread stacks |
| WASM | One `wasm_bindgen_futures::spawn_local` per actor | Allocated local futures/channels in linear memory; toolchain and host control stack/memory limits |
| Embassy | One `#[embassy_executor::task(pool_size = 1)]` per actor **plus one shutdown coordinator per system** | Compiler-sized static task pools; linker RAM layout and platform executor/interrupt stacks |

Each actor handles one message at a time. A larger mailbox does not increase
handler parallelism. Its envelope is an enum large enough for its largest route
(including reply handles, discriminant and alignment). Capacity excludes the
active message and messages retained by blocked senders in their own futures.
Changing capacity can indirectly change a generated type's size, but is never a
stack or future-storage budget. Payloads with owned heap allocations add memory
beyond the envelope on allocating backends.

## Tokio: configure the runtime, not the actor

For example, create the runtime before starting a generated system:

```rust,ignore
let runtime = tokio::runtime::Builder::new_multi_thread()
    .worker_threads(2)
    .thread_stack_size(2 * 1024 * 1024)
    .enable_all()
    .build()?;
runtime.block_on(async {
    // System::builder().counter(Counter::default()).start()?;
    // ...
    Ok::<(), Box<dyn std::error::Error>>(())
})?;
```

This sets a stack size for runtime-created threads, **not** a per-actor limit
or a reservation for each future. It does not resize the calling thread used by
a current-thread runtime. Account separately for blocking work and external
threads. Generated tasks may move between workers; heap-backed future storage
is additional to stacks. Stack overflow is not a mailbox error and may abort the
process; do not depend on panic recovery for it.

The system retains per-actor completion watch receivers and shutdown watch state,
not join handles, with no separate spawned shutdown task. Shutdown closes
admission, lets the active handler finish and discards queued messages;
actor panics are reported as `ActorTaskError`. A handler that never completes can
prevent shutdown. Each request creates a Tokio oneshot reply channel; there is
no framework-wide bound on concurrent callers or their allocated reply storage.
`request_capacity` does not bound Tokio requests.

## WASM: local tasks, no per-actor stack

Generated tasks run locally through the JavaScript event loop. Future/channel
allocations use WASM linear memory; toolchain/linker settings and host support
constrain linear memory and stack limits, not an actor option. Increasing linear
memory does not necessarily increase the stack. Synchronous recursion can exhaust
the stack; memory exhaustion or traps are not converted to route errors.

There is no separate generated lifecycle task: actors signal a completion channel
with capacity equal to the actor count. Shutdown closes mailboxes and waits for
those signals. Panics/traps are not recovered and may prevent completion. Each
request allocates a capacity-one reply channel; concurrent callers and their
futures remain independently unbounded. `request_capacity` does not apply here.

## Embassy: compiler-sized static future storage

The repository currently locks `embassy-executor` 0.10.0 and task macros 0.8.0
(check the example's lockfile too when building it independently). On stable,
Embassy's `#[task]` expansion emits a private static `POOL: TaskPoolHolder` whose
size and alignment are computed at compile time from the **actual generated
future**. It represents `raw::TaskPool<F, N>`, an array of
`raw::TaskStorage<F>`: executor task header plus future storage and padding, not
just `size_of::<F>()`. The nightly expansion uses a typed static `TaskPool`
directly. This version does not require a user-sized task arena.

For a system with A actors, budget A actor pools of size one and one shutdown
pool of size one. Each actor future includes its owned actor state, receiver,
route context, active handler future and any values retained across awaits.
The coordinator retains context and sends shutdown sentinels sequentially.
Also budget one static Embassy channel per actor, one static `Lifecycle`, and
one static `ReplyPool<Response, N>` per command route. Executor/main tasks,
application caller tasks, driver buffers, other statics and interrupt stacks are
**additional** resources.

No actor byte-budget option is added: Embassy already derives the correct size.
Its generated future types and pool statics are private implementation details;
a robust public per-actor byte assertion would require an upstream API rather
than copying expansion internals or guessing from the handler future alone.
Raising `pool_size` would allocate more instances, not enlarge a future's
storage; generated systems are single-start and do not expose that setting.

### Reproducible target RAM check

Use the existing [Micro:bit example](../examples/embassy-microbit-v2/README.md)
as a concrete system: its `request_capacity = 2` reserves two reply slots for
that command, independently of its mailbox and the actor's single task slot.

1. Build the actual target, features, lockfile and release profile used for
   deployment. To inspect the macro chain, use `cargo expand` (optional external
   tool) on that same package/target: find `__<system>_<actor>_task`,
   `__<system>_shutdown_task` and their Embassy `POOL` statics. Expansion explains
   types and counts; it is not a measurement of optimized RAM.
2. For example, from `examples/embassy-microbit-v2`, produce a map with:

   ```sh
   cargo rustc --release --target thumbv7em-none-eabihf -- \
     -C link-arg=-Map=actor-memory.map
   ```

   Inspect the ELF using target-compatible `llvm-size -A <elf>` and
   `llvm-nm --print-size --size-sort --radix=d --demangle <elf>` (or GNU ARM
   equivalents). Inspect the map for generated task `POOL`s, channels, lifecycle
   and reply pools. Names/section placement can change with optimization/LTO;
   use the map and section totals together, not symbol-name matching alone.
3. Record `.bss`, `.data`, other RAM sections, alignment and reserved stack RAM;
   compare against the linker MEMORY regions. Keep a baseline and enforce a
   project-specific RAM budget in CI. Rebuild after changes to actor state,
   handler locals/awaits, message/response types, features, compiler or Embassy.
   A controlled increase in a buffer retained across an await should be visible
   in the relevant pool/total (unless optimized away).
4. Reserve and separately validate worst-case executor polling/call stack and
   nested interrupt stack usage with the platform's stack watermark/guard tools
   and stress tests. Static future size does **not** bound stack usage. A linker
   only catches RAM-region overflow according to the linker script; it cannot
   prove that a dynamically growing stack never collides with statics. Include
   startup temporaries while moving states into task storage in this assessment.

#### Example: a 10 KiB local buffer

```rust,ignore
async fn handle(&mut self, message: Message) {
    let buffer = [0_u8; 10 * 1024];
    use_buffer(&buffer).await;
}
```

Because `buffer` is borrowed after `.await`, it is part of the handler future
and therefore retained in the actor future while that future is suspended.
On Embassy this increases the actor's statically reserved task storage by roughly
10 KiB (plus future/enum/alignment overhead; optimization may change the final
artifact). It is **not** a 10 KiB Embassy stack reservation. On Tokio/WASM it
similarly enlarges the heap/linear-memory future allocation, not a per-actor
stack. Measure the target build as described above.

If the buffer is used and dies entirely before the first `.await`, it need not
be retained in the future; it instead contributes to synchronous stack use
while that code runs. A compiler may optimize storage, so source-level array
length alone is not a reliable final byte measurement. Avoid large stack locals
on embedded targets: put long-lived buffers in explicitly budgeted state/static
storage or process data in smaller chunks. Static storage still consumes RAM;
it simply moves the budget from task storage to another RAM section.

These are deterministic build-artifact measurements, not portable byte counts.
A static RAM overflow normally fails linking; target stack overflow can corrupt
memory or fault depending on platform guards. Neither becomes `SendError`.

### Pool, lifecycle and request failures

Startup reserves all actor and coordinator task tokens before spawning any.
An occupied task pool yields `StartError::TaskCapacityExhausted`, not a request
queue and not a byte-storage overflow. A successful start permanently marks the
static system started; another start returns `AlreadyStarted`, even after
shutdown. Completing tasks does not return static RAM to an allocator. Shutdown
closes admission, then the coordinator enqueues sentinels behind accepted work;
a full mailbox delays it. A stuck handler or panic has no recovery guarantee
and can prevent shutdown. Dropping the Embassy handle alone does not request
shutdown; call `shutdown().await` explicitly.

`#[actor_handler(request_capacity = N)]` (default 1, nonzero) sizes each command
route's static reply pool, **not** its task pool or mailbox. Include response
storage, synchronization and alignment per slot. Callers await a free slot
before sending; that wait is not currently raced against shutdown. Recursive
requests to the same actor, or dependency cycles while slots/mailboxes are
occupied, can deadlock. More slots do not add handler concurrency. Dropping a
requester does not cancel accepted handling: its slot remains reserved until
the actor replies or drops the envelope. A dropped reply sender produces
`RequestError::ResponseDropped`; closed admission produces `RequestError::Closed`.
Pending caller futures require their own task storage even when all reply slots
are busy. Do not count bounded reply slots as a bound on the number of waiting
callers.

## Backpressure is not memory-overflow recovery

Across backends, full mailboxes make `send` wait and `try_send` return `Full`;
closed routes reject new work. Accepted sends acknowledge enqueueing, not
completion. Event fan-out is sequential and can deliver only a prefix on error.
Bound callers, payload sizes and request dependencies as well as queue lengths.
The shared `no_std` core remains allocator- and executor-independent; these
backend allocations and stack policies are not core requirements.
