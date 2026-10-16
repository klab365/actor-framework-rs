#![cfg(feature = "tokio")]

use actor_framework::{ActorContext, actor_system};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Debug, PartialEq, Eq)]
struct CounterChanged(u32);

struct CounterState {
    value: u32,
}

struct ObserverState {
    observed: Arc<Mutex<Vec<u32>>>,
}

async fn increment(context: &ActuatorContext, state: &mut CounterState, amount: u32) {
    state.value += amount;
    context.publish(CounterChanged(state.value)).await.unwrap();
}

async fn observe(_context: &impl ActorContext, state: &mut ObserverState, event: CounterChanged) {
    state.observed.lock().await.push(event.0);
}

actor_framework::actor_module! {
    pub manifest counter_module;
events: [CounterChanged];
    actors: [
        counter: CounterState {
            mailbox_capacity: 4;
            commands { u32 => increment; }
            events {}
        },
        observer_a: ObserverState {
            mailbox_capacity: 4;
            commands {}
            events { CounterChanged => observe; }
        },
        observer_b: ObserverState {
            mailbox_capacity: 4;
            commands {}
            events { CounterChanged => observe; }
        }
    ];
}

actor_system! {
    pub struct AppActors {
        modules: [counter_module];
    }
}

static OBSERVED_A: std::sync::LazyLock<Arc<Mutex<Vec<u32>>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(Vec::new())));
static OBSERVED_B: std::sync::LazyLock<Arc<Mutex<Vec<u32>>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(Vec::new())));

#[tokio::test]
async fn sends_command_and_broadcasts_event_to_all_subscribers() {
    let actors = AppActors::builder()
        .counter(CounterState { value: 0 })
        .observer_a(ObserverState {
            observed: OBSERVED_A.clone(),
        })
        .observer_b(ObserverState {
            observed: OBSERVED_B.clone(),
        })
        .start()
        .unwrap();
    actors.send(3_u32).await.unwrap();
    tokio::task::yield_now().await;
    assert_eq!(*OBSERVED_A.lock().await, vec![3]);
    assert_eq!(*OBSERVED_B.lock().await, vec![3]);
}
