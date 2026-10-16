use actor_framework::{actor_module, actor_system};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::prelude::*;

#[derive(Debug)]
pub struct Add(pub u32);

#[derive(Clone)]
pub struct TotalChanged(pub u32);

pub struct Counter(pub Rc<Cell<u32>>);

pub struct Observer {
    pub total: Rc<Cell<u32>>,
    pub completed: actor_framework::wasm::Sender<()>,
}

pub async fn add(context: &impl counters::ActorContext, state: &mut Counter, message: Add) {
    state.0.set(state.0.get() + message.0);
    context.publish(TotalChanged(state.0.get())).await.unwrap();
}

pub async fn observe(_: &impl counters::ActorContext, state: &mut Observer, event: TotalChanged) {
    state.total.set(event.0);
    if event.0 == 5 {
        actor_framework::ActorSendRoute::send_route(&state.completed, ())
            .await
            .unwrap();
    }
}

actor_module! {
    manifest counters;
    context { send: []; publish: [TotalChanged]; }
    events: [TotalChanged];
    actors: [
        counter: Counter {
            mailbox_capacity: 2;
            commands { Add => add; }
            events {}
        },
        observer: Observer {
            mailbox_capacity: 2;
            commands {}
            events { TotalChanged => observe; }
        }
    ];
}

actor_system! {
    pub struct AppActors { modules: [counters]; }
}

/// Run a statically routed actor system; the returned Promise resolves on success.
#[wasm_bindgen]
pub async fn run_example() {
    let total = Rc::new(Cell::new(0));
    let observed = Rc::new(Cell::new(0));
    let (completed, mut completion) = actor_framework::wasm::channel(1).unwrap();
    let actors = AppActors::builder()
        .counter(Counter(total.clone()))
        .observer(Observer {
            total: observed.clone(),
            completed,
        })
        .start()
        .unwrap();
    actors.try_send(Add(2)).unwrap();
    actors.send(Add(3)).await.unwrap();

    // Allow outgoing events to be accepted before closing all destinations.
    // No completion acknowledgement is implied by send().
    assert_eq!(actor_framework::Receiver::receive(&mut completion).await, Some(()));
    actors.shutdown().await.unwrap();
    assert_eq!(total.get(), 5);
    assert_eq!(observed.get(), 5);
    web_sys::console::log_1(&"WASM actor_module!/actor_system! example passed".into());
}
