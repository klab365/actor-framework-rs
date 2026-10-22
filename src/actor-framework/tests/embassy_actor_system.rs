#![cfg(all(feature = "embassy", not(feature = "tokio")))]
#![allow(clippy::redundant_field_names)]

use actor_framework::{actor, actor_system};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use embassy_executor::{Spawner, task};
use static_cell::StaticCell;

static OBSERVED: AtomicUsize = AtomicUsize::new(0);
static STORED: AtomicUsize = AtomicUsize::new(0);
static DONE: AtomicBool = AtomicBool::new(false);
static EXECUTOR: StaticCell<embassy_executor::Executor> = StaticCell::new();

#[derive(actor_framework::ActorCommand)]
#[actor_command(response = ())]
struct Add(usize);
#[derive(actor_framework::ActorCommand)]
#[actor_command(response = ())]
struct Store(usize);
#[derive(Clone)]
struct Changed(usize);
struct Counter;
struct Logger;

impl Counter {
    #[actor_framework::actor_handler(request_capacity = 2)]
    async fn add(
        &mut self,
        stored: ActorAddress<ActorCommand<Store>>,
        events: ActorAddress<ActorEvent<Changed>>,

        message: Add,
    ) {
        stored.post(Store(message.0)).await.unwrap();
        events.publish(Changed(message.0)).await.unwrap();
    }
}

impl Logger {
    #[actor_framework::actor_handler]
    async fn log(&mut self, event: Changed) {
        OBSERVED.fetch_add(event.0, Ordering::SeqCst);
    }
}

impl Logger {
    #[actor_framework::actor_handler(request_capacity = 2)]
    async fn store(&mut self, message: Store) {
        STORED.fetch_add(message.0, Ordering::SeqCst);
    }
}

actor! {
    actor counter: Counter {
        mailbox_capacity: 1;
        commands { Add => Counter::add; }
        events {}
    };
}

actor! {
    actor logger: Logger {
        mailbox_capacity: 1;
        commands { Store => Logger::store; }
        events { Changed => Logger::log; }
    };
}

actor_system! {
    struct TestActors { actors: [counter, logger]; }
}

#[task]
async fn scenario(spawner: Spawner) {
    let actors = TestActors::builder()
        .counter(Counter)
        .logger(Logger)
        .start(spawner)
        .unwrap();
    assert_eq!(actors.request(Add(1)).await, Ok(()));
    actors.send(Add(2)).await.unwrap();
    while OBSERVED.load(Ordering::SeqCst) != 3 || STORED.load(Ordering::SeqCst) != 3 {
        embassy_futures::yield_now().await;
    }
    actors.shutdown().await;
    assert_eq!(OBSERVED.load(Ordering::SeqCst), 3);
    assert_eq!(STORED.load(Ordering::SeqCst), 3);
    DONE.store(true, Ordering::SeqCst);
}

#[test]
fn generated_system_drains_before_shutdown() {
    let executor = EXECUTOR.init(embassy_executor::Executor::new());
    executor.run_until(
        |spawner| spawner.spawn(scenario(spawner).unwrap()),
        || DONE.load(Ordering::SeqCst),
    );
}
