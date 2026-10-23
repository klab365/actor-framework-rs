#![no_std]
#![no_main]

use actor_framework::{actor, actor_handler, actor_system};
use defmt::info;
use defmt_rtt as _;
use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_nrf::gpio::{Input, Level, Output, OutputDrive, Pull};
use embassy_time::{Duration, Timer};
use panic_probe as _;

#[derive(Clone, actor_framework::ActorCommand)]
#[actor_command(response = ())]
enum ButtonPressed {
    A,
    B,
}

/// Query command whose typed response is observed through `request()`.
#[derive(actor_framework::ActorCommand)]
#[actor_command(response = Result<u32, ()>)]
struct ReadCount;

/// Ask the counter actor to read itself through an injected request capability.
#[derive(actor_framework::ActorCommand)]
#[actor_command(response = Result<u32, ()>)]
struct ReportCount;

#[derive(Clone)]
struct PressObserved {
    button: ButtonPressed,
    count: u32,
}

struct Counter {
    count: u32,
}

struct ActivityLed {
    column: Output<'static>,
    row: Output<'static>,
}

struct CountReporter;

impl Counter {
    #[actor_handler]
    async fn count_press(&mut self, events: ActorAddress<ActorEvent<PressObserved>>, button: ButtonPressed) {
        self.count = self.count.wrapping_add(1);
        events
            .publish(PressObserved {
                button,
                count: self.count,
            })
            .await
            .expect("event route is open");
    }
}

/// Reserve two static reply slots so independent callers may query together.
impl Counter {
    #[actor_handler(request_capacity = 2)]
    async fn read_count(&mut self, _command: ReadCount) -> Result<u32, ()> {
        Ok(self.count)
    }
}

impl CountReporter {
    #[actor_handler]
    async fn report(
        &mut self,
        counter: ActorAddress<ActorRequest<ReadCount>>,
        _command: ReportCount,
    ) -> Result<u32, ()> {
        info!("reporter requesting count from counter actor");
        let result = counter.request(ReadCount).await;
        match &result {
            Ok(Ok(count)) => info!("reporter received count {}", count),
            Ok(Err(())) => info!("counter rejected reporter request"),
            Err(_) => info!("reporter request failed: counter stopped before replying"),
        }
        result.expect("counter actor stopped")
    }
}

impl ActivityLed {
    #[actor_handler]
    async fn blink(&mut self, event: PressObserved) {
        if self.column.is_set_low() {
            self.column.set_high();
            self.row.set_low();
        } else {
            self.row.set_high();
            self.column.set_low();
        }
        match event.button {
            ButtonPressed::A => info!("button A pressed; count is {}", event.count),
            ButtonPressed::B => info!("button B pressed; count is {}", event.count),
        }
        Timer::after(Duration::from_millis(100)).await;
        self.row.set_high();
        self.column.set_low();
    }
}

actor! {
    actor counter: Counter {
        mailbox_capacity: 4;
        commands { ButtonPressed => Counter::count_press;, ReadCount => Counter::read_count; }
        events {}
    };
}

actor! {
    actor reporter: CountReporter {
        mailbox_capacity: 2;
        commands { ReportCount => CountReporter::report; }
        events {}
    };
}

actor! {
    actor activity_led: ActivityLed {
        mailbox_capacity: 2;
        commands {}
        events { PressObserved => ActivityLed::blink; }
    };
}

actor_system! {
    pub struct AppActors {
        actors: [counter, reporter, activity_led];
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let peripherals = embassy_nrf::init(Default::default());
    let mut button_a = Input::new(peripherals.P0_14, Pull::Up);
    let mut button_b = Input::new(peripherals.P0_23, Pull::Up);
    let actors = AppActors::builder()
        .counter(Counter { count: 0 })
        .reporter(CountReporter)
        .activity_led(ActivityLed {
            // P0.28/P0.21 drive one micro:bit-v2 LED matrix pixel.
            column: Output::new(peripherals.P0_28, Level::High, OutputDrive::Standard),
            row: Output::new(peripherals.P0_21, Level::Low, OutputDrive::Standard),
        })
        .start(spawner)
        .expect("actor task capacity exhausted");

    info!("typed actor-address micro:bit example started");
    loop {
        let button = match select(button_a.wait_for_low(), button_b.wait_for_low()).await {
            Either::First(_) => {
                button_a.wait_for_high().await;
                ButtonPressed::A
            }
            Either::Second(_) => {
                button_b.wait_for_high().await;
                ButtonPressed::B
            }
        };
        // `send` only confirms bounded-mailbox acceptance; it does not wait
        // for `count_press` to execute or return a value.
        actors.send(button).await.expect("counter actor stopped");

        // `request` waits for the typed handler response. Handler errors stay
        // inside the outer transport result.
        match actors.request(ReportCount).await {
            Ok(Ok(count)) => info!("reporter queried counter; count is {}", count),
            Ok(Err(())) => info!("counter rejected read"),
            Err(_) => info!("reporter or counter stopped before replying"),
        }
        Timer::after(Duration::from_millis(50)).await;
    }
}
