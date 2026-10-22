#![cfg(feature = "tokio")]

use actor_framework::actor_system;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Debug, PartialEq, Eq)]
struct CounterChanged(u32);

#[derive(actor_framework::ActorCommand)]
#[actor_command(response = ())]
struct Amount(u32);

struct CounterState {
    value: u32,
}

struct ObserverState {
    observed: Arc<Mutex<Vec<u32>>>,
}

impl CounterState {
    #[actor_framework::actor_handler]
    async fn increment(&mut self, events: ActorAddress<ActorEvent<CounterChanged>>, amount: Amount) {
        self.value += amount.0;
        events.publish(CounterChanged(self.value)).await.unwrap();
    }
}

impl ObserverState {
    #[actor_framework::actor_handler]
    async fn observe(&mut self, event: CounterChanged) {
        self.observed.lock().await.push(event.0);
    }
}

actor_framework::actor! {
    pub actor counter: CounterState {
        mailbox_capacity: 4;
        commands { Amount => CounterState::increment; }
        events {}
    };
}

actor_framework::actor! {
    pub actor observer_a: ObserverState {
        mailbox_capacity: 4;
        commands {}
        events { CounterChanged => ObserverState::observe; }
    };
}

actor_framework::actor! {
    pub actor observer_b: ObserverState {
        mailbox_capacity: 4;
        commands {}
        events { CounterChanged => ObserverState::observe; }
    };
}

actor_system! {
    pub struct AppActors {
        actors: [counter, observer_a, observer_b];
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
    actors.send(Amount(3)).await.unwrap();
    tokio::task::yield_now().await;
    assert_eq!(*OBSERVED_A.lock().await, vec![3]);
    assert_eq!(*OBSERVED_B.lock().await, vec![3]);
}
