use actor_framework::{actor_handler, actor_module, actor_system};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(actor_framework::ActorCommand)]
#[actor_command(response = ())]
struct Add(u32);

#[derive(Clone)]
struct TotalChanged(u32);

struct Counter {
    total: u32,
}

struct Logger {
    values: Arc<Mutex<Vec<u32>>>,
}

#[actor_handler]
async fn add(events: ActorAddress<ActorEvent<TotalChanged>>, counter: &mut Counter, message: Add) {
    println!("[counter] received Add({})", message.0);
    counter.total += message.0;
    events
        .publish(TotalChanged(counter.total))
        .await
        .expect("event published");
    println!("[counter] total is {}", counter.total);
}

#[actor_handler]
async fn log(logger: &mut Logger, event: TotalChanged) {
    println!("[logger] received TotalChanged({})", event.0);
    logger.values.lock().await.push(event.0);
}

static LOG: std::sync::LazyLock<Arc<Mutex<Vec<u32>>>> = std::sync::LazyLock::new(|| Arc::new(Mutex::new(Vec::new())));

actor_module! {
    pub manifest application;
    events: [TotalChanged];
    actors: [
        counter: Counter {
            mailbox_capacity: 8;
            commands { Add => add; }
            events {}
        },
        logger: Logger {
            mailbox_capacity: 8;
            commands {}
            events { TotalChanged => log; }
        }
    ];
}

actor_system! {
    pub struct AppActors {
        modules: [application];
    }
}

#[tokio::main]
async fn main() {
    println!("[main] starting actor system");
    let actors = AppActors::builder()
        .counter(Counter { total: 0 })
        .logger(Logger { values: LOG.clone() })
        .start()
        .expect("all actor states supplied");

    println!("[main] sending Add(5)");
    actors.send(Add(5)).await.expect("counter is running");
    println!("[main] sending Add(7)");
    actors.send(Add(7)).await.expect("counter is running");

    for _ in 0..20 {
        if LOG.lock().await.as_slice() == [5, 12] {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }

    let observed = LOG.lock().await.clone();
    assert_eq!(observed, [5, 12]);
    println!("Observed totals: {observed:?}");
}
