# Embassy micro:bit v2 actor-system example

A flashable BBC micro:bit v2 / nRF52833 example of typed Embassy actor
addresses. Button A (`P0.14`) and Button B (`P0.23`) are awaited together and
send a typed `ButtonPressed::{A, B}` command to a statically allocated counter
mailbox. Its handler declares `ActorAddress<ActorEvent<PressObserved>>` and
publishes the button identity and total to the LED actor; the generated system
wires that typed context. The LED actor toggles the matrix pixel at
`P0.28`/`P0.21` and logs through defmt. Messages and mailboxes remain statically typed; there is no registry,
serialization, or heap allocation.

Mailbox capacities are Counter=4 and LED=2. A full mailbox backpressures
`send`/`publish`; success means the message was accepted, not handled. The
`#[actor_handler]` attribute derives typed route requirements from handler
parameters; `actor_system!` binds them to the composed system. This firmware
intentionally keeps the executor running.

## Flash

```sh
rustup target add thumbv7em-none-eabihf
cargo install probe-rs-tools
cd examples/embassy-microbit-v2
cargo run --release
```

Press Button A or B. `defmt-rtt` identifies the pressed button, reports the
shared counter, and the activity pixel flashes.
