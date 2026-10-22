#![cfg(all(feature = "wasm", target_arch = "wasm32"))]

use actor_framework::{ActorSendRoute, Receiver, TrySendError, actor_module, actor_system, wasm};
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

#[actor_framework::actor_handler]
pub async fn add(state: &mut Counter, command: Add) {
    state.0.set(state.0.get() + command.0);
    // Shutdown closes every destination: outgoing routes may already be closed.
}
#[actor_framework::actor_handler]
pub async fn read_total(state: &mut Counter, _: ReadTotal) -> Result<u32, &'static str> {
    Ok(state.0.get())
}
#[actor_framework::actor_handler]
pub async fn observe(state: &mut Observer, event: Changed) {
    state.0.send_route(event.0).await.unwrap();
}

actor_module! {
    pub manifest counters;
    actors: [
        counter: Counter {
            mailbox_capacity: capacity();
            commands { Add => add;, ReadTotal => read_total; }
            events { Unobserved => []; }
        },
        first: Observer {
            mailbox_capacity: 1;
            commands {}
            events { Changed => observe; }
        },
        second: Observer {
            mailbox_capacity: 1;
            commands {}
            events { Changed => observe; }
        }
    ];
}
actor_system! { pub struct App { modules: [counters]; } }

#[wasm_bindgen_test]
async fn manifests_routes_builder_backpressure_and_shutdown() {
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
