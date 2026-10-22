# When Are Actors Useful?

This brief guide complements the [installation and getting-started guide](../README.md);
it does not describe additional framework features.

The actor model is a concurrency architecture in which each actor owns its
state, processes messages sequentially, and communicates through messages
rather than shared mutable state. It is not an end in itself and is not
automatically faster or simpler than ordinary functions.

## Actors are a good fit when…

- **State has a clear owner.** A component manages a connection, device channel,
  or session. Other components send commands instead of directly mutating its
  state.
- **Many independent workflows are active.** Connections, sessions, or devices
  can receive messages and make progress independently.
- **Events and commands arrive asynchronously.** Network, UI, sensor, timer, or
  interrupt-adjacent inputs can be modeled as messages.
- **Components should be decoupled.** Senders need not know the receiver's
  implementation. Runtime adapters can process the same domain messages.
- **Failure and lifecycle boundaries matter.** A component may stop or shut down
  independently, while other components do not access its state directly.
- **Backpressure matters.** Bounded mailboxes make it visible when consumers
  cannot keep up, provided overflow and send behavior are explicit.

## Typical use cases

| Area | Possible actors | Why the model can help |
| --- | --- | --- |
| Network servers | One actor per connection or session | Connection state remains local; messages serialize state changes. |
| Devices and IoT | One actor per device, sensor, or bus channel | Independent device workflows and event processing can be isolated. |
| UI and applications | One actor per document, window, or background task | Long-running work and UI events can be coordinated through messages. |
| Near-real-time systems | One actor per control function or resource | Responsibilities and message flows are explicit; hard real-time needs additional guarantees. |
| Distributed systems | One actor per partition, session, or service instance | The local messaging model may be a foundation for distribution, but is not a distributed protocol. |

## When actors are probably not a good fit

- For simple synchronous computations without shared state, a function call is
  usually clearer and cheaper.
- If two steps must wait for each other immediately and no independent work is
  possible, messages and mailboxes add unnecessary complexity.
- If data is large, immutable, and easy to share, copying or forwarding many
  messages may be expensive.
- Actor boundaries do not make transactions across multiple states atomic.
- Extremely low latency or deterministic execution requires mailboxes, the
  scheduler, and memory behavior to be designed for those requirements.

## What actors do not solve automatically

Messages can be delayed, sends can fail, mailboxes can fill, and consumers can
remain overloaded. Restart strategies, ordering, delivery guarantees, timeout
behavior, and shutdown need explicit design. Successful mailbox delivery does
not necessarily mean that a message was processed or durably stored.

## Implications across Rust runtimes

A framework targeting Tokio/std, WebAssembly, and Embassy should separate the
actor model from its execution. Domain logic and messages can remain runtime
neutral, while scheduling, timers, synchronization, and mailbox transport vary
by platform. Tokio/std suits general-purpose servers and applications; WASM
runs under its host constraints; Embassy targets resource-constrained embedded
systems. A shared API should not assume that every platform has threads, `std`,
dynamic allocation, or the same async primitives.

## Rule of thumb

Use actors when a component **owns state**, **processes messages independently**,
and forms a **clear concurrency or isolation boundary**. Otherwise, begin with
ordinary functions and data structures.
