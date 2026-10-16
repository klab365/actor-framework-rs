# Embassy micro:bit v2 actor-system example

A flashable BBC micro:bit v2 / nRF52833 example using the generated Embassy
`actor_system!` backend. Button A (`P0.14`) sends `ButtonPressed` to the Counter
actor. It publishes `PressObserved`; the LED actor toggles the matrix pixel at
`P0.28`/`P0.21` and logs through defmt.

The application only starts the generated system once:

```rust,ignore
let actors = AppActors::builder()
    .counter(Counter { count: 0 })
    .activity_led(activity_led)
    .start(spawner)?;
```

Mailbox capacities are Counter=4 and LED=2. The generated task pool has one
static task slot per actor plus one private shutdown-coordinator slot. A full mailbox backpressures `send`; the button loop
waits for acceptance, not completion. Generated systems also provide
`shutdown().await`: it rejects new sends, drains accepted messages, and waits
for both actor tasks. This firmware intentionally keeps the executor running.

## Flash

```sh
rustup target add thumbv7em-none-eabihf
cargo install probe-rs-tools
cd examples/embassy-microbit-v2
cargo run --release
```

Press Button A. `defmt-rtt` reports the counter and the activity pixel flashes.
