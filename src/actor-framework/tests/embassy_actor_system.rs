#![cfg(all(feature = "embassy", not(feature = "tokio")))]

use actor_framework::{actor_module, actor_system};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use embassy_executor::{Spawner, task};
use static_cell::StaticCell;

static OBSERVED: AtomicUsize = AtomicUsize::new(0);
static DONE: AtomicBool = AtomicBool::new(false);
static EXECUTOR: StaticCell<embassy_executor::Executor> = StaticCell::new();

struct Add(usize);
struct Counter;

async fn add(_actor: &mut Counter, message: Add) {
    OBSERVED.fetch_add(message.0, Ordering::SeqCst);
}

actor_module! {
    manifest test_manifest;
    events: [];
    actors: [
        counter: Counter {
            mailbox_capacity: 1;
            commands { Add => add; }
            events {}
        }
    ];
}

actor_system! {
    struct TestActors { modules: [test_manifest]; }
}

#[task]
async fn scenario(spawner: Spawner) {
    let actors = TestActors::builder().counter(Counter).start(spawner).unwrap();
    actors.send(Add(1)).await.unwrap();
    actors.send(Add(2)).await.unwrap();
    actors.shutdown().await;
    assert_eq!(OBSERVED.load(Ordering::SeqCst), 3);
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
