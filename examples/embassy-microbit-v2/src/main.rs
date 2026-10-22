#![no_std]
#![no_main]

use actor_framework::{actor, actor_handler, actor_system};
use defmt::info;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_nrf::gpio::{Input, Level, Output, OutputDrive, Pull};
use embassy_time::{Duration, Timer};
use panic_probe as _;

#[derive(actor_framework::ActorCommand)]
#[actor_command(response = ())]
struct ButtonPressed;

/// Query command whose typed response is observed through `request()`.
#[derive(actor_framework::ActorCommand)]
#[actor_command(response = Result<u32, ()>)]
struct ReadCount;

#[derive(Clone)]
struct PressObserved(u32);

struct Counter {
    count: u32,
}

struct ActivityLed {
    column: Output<'static>,
    row: Output<'static>,
}

#[actor_handler]
async fn count_press(events: ActorAddress<ActorEvent<PressObserved>>, counter: &mut Counter, _message: ButtonPressed) {
    counter.count = counter.count.wrapping_add(1);
    events
        .publish(PressObserved(counter.count))
        .await
        .expect("event route is open");
}

/// Reserve two static reply slots so independent callers may query together.
#[actor_handler(request_capacity = 2)]
async fn read_count(counter: &mut Counter, _command: ReadCount) -> Result<u32, ()> {
    Ok(counter.count)
}

#[actor_handler]
async fn blink(led: &mut ActivityLed, event: PressObserved) {
    if led.column.is_set_low() {
        led.column.set_high();
        led.row.set_low();
    } else {
        led.row.set_high();
        led.column.set_low();
    }
    info!("button A pressed {} times", event.0);
    Timer::after(Duration::from_millis(100)).await;
    led.row.set_high();
    led.column.set_low();
}

actor! {
    actor counter: Counter {
        mailbox_capacity: 4;
        commands { ButtonPressed => count_press;, ReadCount => read_count; }
        events {}
    };
}

actor! {
    actor activity_led: ActivityLed {
        mailbox_capacity: 2;
        commands {}
        events { PressObserved => blink; }
    };
}

actor_system! {
    pub struct AppActors {
        actors: [counter, activity_led];
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let peripherals = embassy_nrf::init(Default::default());
    let mut button = Input::new(peripherals.P0_14, Pull::Up);
    let actors = AppActors::builder()
        .counter(Counter { count: 0 })
        .activity_led(ActivityLed {
            // P0.28/P0.21 drive one micro:bit-v2 LED matrix pixel.
            column: Output::new(peripherals.P0_28, Level::High, OutputDrive::Standard),
            row: Output::new(peripherals.P0_21, Level::Low, OutputDrive::Standard),
        })
        .start(spawner)
        .expect("actor task capacity exhausted");

    info!("typed actor-address micro:bit example started");
    loop {
        button.wait_for_low().await;
        // `send` only confirms bounded-mailbox acceptance; it does not wait
        // for `count_press` to execute or return a value.
        actors.send(ButtonPressed).await.expect("counter actor stopped");

        // `request` waits for the typed handler response. Handler errors stay
        // inside the outer transport result.
        match actors.request(ReadCount).await {
            Ok(Ok(count)) => info!("counter now reads {}", count),
            Ok(Err(())) => info!("counter rejected read"),
            Err(_) => info!("counter actor stopped before replying"),
        }
        button.wait_for_high().await;
        Timer::after(Duration::from_millis(50)).await;
    }
}
