#![cfg(feature = "wasm")]

use actor_framework::{ActorPublishRoute, ActorSendRoute, Receiver, SendError, TrySendError, wasm};
use std::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};

#[test]
fn bounded_cancellation_and_lifecycle() {
    assert!(wasm::channel::<u8>(0).is_err());
    let (sender, mut receiver) = wasm::channel(1).unwrap();
    let mut cx = Context::from_waker(Waker::noop());
    sender.try_send(1).unwrap();
    assert_eq!(sender.try_send(2), Err(TrySendError::Full(2)));
    {
        let mut pending = pin!(sender.send_route(3));
        assert!(pending.as_mut().poll(&mut cx).is_pending());
    }
    assert_eq!(futures_lite::future::block_on(receiver.receive()), Some(1));
    {
        let mut pending = pin!(receiver.receive());
        assert!(pending.as_mut().poll(&mut cx).is_pending());
    }
    sender.try_send(4).unwrap();
    sender.close();
    assert_eq!(sender.try_send(5), Err(TrySendError::Closed(5)));
    assert_eq!(futures_lite::future::block_on(receiver.receive()), Some(4));
    assert_eq!(futures_lite::future::block_on(receiver.receive()), None);
    assert_eq!(futures_lite::future::block_on(sender.send_route(6)), Err(SendError));
}

#[test]
fn backpressure_wakes_and_fanout_is_partial() {
    futures_lite::future::block_on(async {
        let (sender, mut receiver) = wasm::channel(1).unwrap();
        sender.try_send(1).unwrap();
        let send = async { sender.send_route(2).await.unwrap() };
        let receive = async {
            assert_eq!(receiver.receive().await, Some(1));
            assert_eq!(receiver.receive().await, Some(2));
        };
        futures_lite::future::zip(send, receive).await;
        let (closed, other) = wasm::channel(1).unwrap();
        drop(other);
        let publisher = wasm::Publisher([sender.clone(), closed]);
        assert_eq!(publisher.publish_route(3).await, Err(SendError));
        assert_eq!(receiver.receive().await, Some(3));
        wasm::Publisher::<u8, 0>([]).publish_route(0).await.unwrap();
        drop(sender);
        drop(publisher);
        assert_eq!(receiver.receive().await, None);
    });
}

#[test]
fn closure_wakes_pending_send() {
    let (sender, receiver) = wasm::channel(1).unwrap();
    sender.try_send(1).unwrap();
    let mut pending = pin!(sender.send_route(2));
    let mut cx = Context::from_waker(Waker::noop());
    assert!(pending.as_mut().poll(&mut cx).is_pending());
    drop(receiver);
    assert_eq!(pending.as_mut().poll(&mut cx), Poll::Ready(Err(SendError)));
}

#[cfg(target_arch = "wasm32")]
mod js {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    struct Counter(Rc<Cell<u8>>, wasm::Sender<u8>);

    impl actor_framework::Actor<u8, ()> for Counter {
        async fn handle(&mut self, _: &(), message: u8) {
            self.0.set(self.0.get() + message);
            self.1.send_route(self.0.get()).await.unwrap();
        }
    }

    #[wasm_bindgen_test::wasm_bindgen_test]
    async fn local_actor_runs_on_js_executor() {
        let total = Rc::new(Cell::new(0));
        let (commands, receiver) = wasm::channel(1).unwrap();
        let (results, mut observed) = wasm::channel(1).unwrap();
        wasm::spawn_actor(Counter(total.clone(), results), (), receiver);
        commands.send_route(2).await.unwrap();
        commands.send_route(3).await.unwrap();
        commands.close();
        assert_eq!(observed.receive().await, Some(2));
        assert_eq!(observed.receive().await, Some(5));
        assert_eq!(observed.receive().await, None);
        assert_eq!(total.get(), 5);
    }
}
