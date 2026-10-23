use actor_framework::{actor_handler, actor, actor_system};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(actor_framework::ActorCommand)]
#[actor_command(response = ())]
struct Add(u32);

#[derive(actor_framework::ActorCommand)]
#[actor_command(response = Result<u32, ()>)]
struct GetTotal;

#[derive(actor_framework::ActorCommand)]
#[actor_command(response = Result<u32, ()>)]
struct Audit;

#[derive(Clone)]
struct TotalChanged(u32);

struct Counter {
    total: u32,
}

struct Logger {
    values: Arc<Mutex<Vec<u32>>>,
}

/// Demonstrates an actor awaiting a typed response from another actor.
struct Auditor;

impl Counter {
#[actor_handler]
async fn add(&mut self, events: ActorAddress<ActorEvent<TotalChanged>>,  message: Add) {
    println!("[counter] received Add({})", message.0);
    self.total += message.0;
    events
        .publish(TotalChanged(self.total))
        .await
        .expect("event published");
    println!("[counter] total is {}", self.total);
}
}

impl Counter {
    #[actor_handler]
    async fn get_total(&mut self, _: GetTotal) -> Result<u32, ()> {
        Ok(self.total)
    }
}

impl Logger {
#[actor_handler]
async fn log(&mut self, event: TotalChanged) {
    println!("[logger] received TotalChanged({})", event.0);
    self.values.lock().await.push(event.0);
}
}

impl Auditor {
    #[actor_handler]
    async fn audit(
        &mut self,
        counter: ActorAddress<ActorRequest<GetTotal>>,
        _: Audit,
    ) -> Result<u32, ()> {
        counter.request(GetTotal).await.expect("counter is running")
    }
}

static LOG: std::sync::LazyLock<Arc<Mutex<Vec<u32>>>> = std::sync::LazyLock::new(|| Arc::new(Mutex::new(Vec::new())));

actor! {
    pub actor counter: Counter {
        mailbox_capacity: 8;
        commands { Add => Counter::add;, GetTotal => Counter::get_total; }
        events {}
    };
}

actor! {
    pub actor logger: Logger {
        mailbox_capacity: 8;
        commands {}
        events { TotalChanged => Logger::log; }
    };
}

actor! {
    pub actor auditor: Auditor {
        mailbox_capacity: 2;
        commands { Audit => Auditor::audit; }
        events {}
    };
}

actor_system! {
    pub struct AppActors {
        actors: [counter, logger, auditor];
    }
}

#[tokio::main]
async fn main() {
    println!("[main] starting actor system");
    let actors = AppActors::builder()
        .counter(Counter { total: 0 })
        .logger(Logger { values: LOG.clone() })
        .auditor(Auditor)
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
    // Auditor receives `Audit`, then uses an injected ActorRequest capability
    // to await Counter's typed GetTotal response.
    assert_eq!(actors.request(Audit).await, Ok(Ok(12)));
    println!("Observed totals: {observed:?}");
}
