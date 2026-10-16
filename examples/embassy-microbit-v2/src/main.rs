#![no_std]
#![no_main]

use actor_framework::{actor_module, actor_system};
use defmt::info;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_nrf::gpio::{Input, Level, Output, OutputDrive, Pull};
use embassy_time::{Duration, Timer};
use panic_probe as _;

struct ButtonPressed;
#[derive(Clone)]
struct PressObserved(u32);

struct Counter {
    count: u32,
}

struct ActivityLed {
    column: Output<'static>,
    row: Output<'static>,
}

impl ActivityLed {
    fn toggle(&mut self) {
        if self.column.is_set_low() {
            self.column.set_high();
            self.row.set_low();
        } else {
            self.row.set_high();
            self.column.set_low();
        }
    }
}

async fn count_press(context: &ActuatorContext, counter: &mut Counter, _: ButtonPressed) {
    counter.count = counter.count.wrapping_add(1);
    context
        .publish(PressObserved(counter.count))
        .await
        .expect("LED actor is running");
}

async fn blink(_context: &ActuatorContext, led: &mut ActivityLed, event: PressObserved) {
    led.toggle();
    info!("micro:bit button A pressed {} times", event.0);
    Timer::after(Duration::from_millis(100)).await;
    led.toggle();
}

actor_module! {
    manifest buttons;
    events: [PressObserved];
    actors: [
        counter: Counter {
            mailbox_capacity: 4;
            commands { ButtonPressed => count_press; }
            events {}
        },
        activity_led: ActivityLed {
            mailbox_capacity: 2;
            commands {}
            events { PressObserved => blink; }
        }
    ];
}

actor_system! {
    pub struct AppActors {
        modules: [buttons];
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

    info!("actor-framework Embassy micro:bit v2 example started");
    loop {
        button.wait_for_low().await;
        actors.send(ButtonPressed).await.expect("counter task stopped");
        button.wait_for_high().await;
        Timer::after(Duration::from_millis(50)).await;
    }
}
