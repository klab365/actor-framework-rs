#![no_std]
#![no_main]

use actor_framework::{actor_module, actor_system};
use embassy_executor::Spawner;
use panic_halt as _;

struct Add(u32);
#[derive(Clone)]
struct TotalChanged(u32);
struct Counter(u32);
struct Logger;

#[actor_framework::actor_handler]
async fn add(counter: &mut Counter, message: Add) {
    counter.0 = counter.0.wrapping_add(message.0);
}

#[actor_framework::actor_handler]
async fn log(_logger: &mut Logger, event: TotalChanged) {
    let _observed = event.0;
}

actor_module! {
    manifest app;
    events: [TotalChanged];
    actors: [
        counter: Counter {
            mailbox_capacity: 4;
            commands { Add => add; }
            events {}
        },
        logger: Logger {
            mailbox_capacity: 2;
            commands {}
            events { TotalChanged => log; }
        }
    ];
}

actor_system! {
    pub struct AppActors {
        modules: [app];
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let actors = AppActors::builder()
        .counter(Counter(0))
        .logger(Logger)
        .start(spawner)
        .expect("actor task capacity exhausted");
    for value in 1..=8 {
        actors.send(Add(value)).await.expect("counter stopped");
    }
}
