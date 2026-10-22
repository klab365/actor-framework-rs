#![cfg(all(feature = "wasm", target_arch = "wasm32"))]

use actor_framework::{ActorSendRoute, Receiver, TrySendError, actor, actor_system, wasm};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen_test::wasm_bindgen_test;

#[derive(Debug, actor_framework::ActorCommand)]
#[actor_command(response = ())]
pub struct Add(pub u32);
#[derive(actor_framework::ActorCommand)]
#[actor_command(response = Result<u32, &'static str>)]
pub struct ReadTotal;
#[derive(Clone)]
pub struct Changed(pub u32);
#[derive(Clone)]
pub struct Unobserved;
pub struct Counter(pub Rc<Cell<u32>>);
pub struct Observer(pub wasm::Sender<u32>);

thread_local! { static CAPACITY: Cell<usize> = const { Cell::new(1) }; }
fn capacity() -> usize {
    CAPACITY.with(Cell::get)
}

impl Counter {
    #[actor_framework::actor_handler]
    pub async fn add(
        &mut self,
        events: actor_framework::ActorAddress<actor_framework::ActorEvent<Changed>>,
        command: Add,
    ) {
        self.0.set(self.0.get() + command.0);
        // Shutdown closes every destination: outgoing routes may already be closed.
        let _ = events.publish(Changed(self.0.get())).await;
    }
}
impl Counter {
    #[actor_framework::actor_handler]
    pub async fn read_total(&mut self, _: ReadTotal) -> Result<u32, &'static str> {
        Ok(self.0.get())
    }
}
impl Observer {
    #[actor_framework::actor_handler]
    pub async fn observe(&mut self, event: Changed) {
        self.0.send_route(event.0).await.unwrap();
    }
}

actor! {
    pub actor counter: Counter {
        mailbox_capacity: capacity();
        commands { Add => Counter::add;, ReadTotal => Counter::read_total; }
        events { Unobserved => []; }
    };
}

actor! {
    pub actor first: Observer {
        mailbox_capacity: 1;
        commands {}
        events { Changed => Observer::observe; }
    };
}

actor! {
    pub actor second: Observer {
        mailbox_capacity: 1;
        commands {}
        events { Changed => Observer::observe; }
    };
}
actor_system! { pub struct App { actors: [counter, first, second]; } }

#[wasm_bindgen_test]
async fn actors_route_builder_backpressure_and_shutdown() {
    assert!(matches!(
        App::builder().start(),
        Err(wasm::StartError::MissingActorState("counter"))
    ));
    let total = Rc::new(Cell::new(0));
    let (first, mut first_rx) = wasm::channel(4).unwrap();
    let (second, mut second_rx) = wasm::channel(4).unwrap();
    CAPACITY.with(|capacity| capacity.set(0));
    assert!(matches!(
        App::builder()
            .counter(Counter(total.clone()))
            .first(Observer(first.clone()))
            .second(Observer(second.clone()))
            .start(),
        Err(wasm::StartError::InvalidCapacity("counter"))
    ));
    CAPACITY.with(|capacity| capacity.set(1));
    let app = App::builder()
        .counter(Counter(total.clone()))
        .first(Observer(first))
        .second(Observer(second))
        .start()
        .unwrap();
    app.try_send(Add(2)).unwrap();
    assert!(matches!(app.try_send(Add(99)), Err(TrySendError::Full(Add(99)))));
    app.send(Add(3)).await.unwrap();
    assert_eq!(app.request(ReadTotal).await, Ok(Ok(5)));
    // Method-injected typed event routing publishes each accepted Add total.
    for receiver in [&mut first_rx, &mut second_rx] {
        assert_eq!(receiver.receive().await, Some(2));
        assert_eq!(receiver.receive().await, Some(5));
    }
    app.publish(Unobserved).await.unwrap();
    app.publish(Changed(7)).await.unwrap();
    assert_eq!(first_rx.receive().await, Some(7));
    assert_eq!(second_rx.receive().await, Some(7));
    // Accepted commands are drained even if shutdown starts before dispatch.
    app.try_send(Add(4)).unwrap();
    {
        use std::{
            future::Future,
            pin::pin,
            task::{Context, Waker},
        };
        let mut shutdown = pin!(app.shutdown());
        assert!(
            shutdown
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        assert_eq!(app.shutdown().await, Err(wasm::ShutdownInProgress));
        // Cancelling a pending shutdown releases its local exclusion guard.
    }
    app.shutdown().await.unwrap();
    app.shutdown().await.unwrap();
    assert_eq!(total.get(), 9);
    assert!(app.send(Add(1)).await.is_err());
    assert!(matches!(app.try_send(Add(1)), Err(TrySendError::Closed(Add(1)))));
    assert_eq!(first_rx.receive().await, None);
    assert_eq!(second_rx.receive().await, None);
}
