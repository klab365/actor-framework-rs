#![cfg(feature = "embassy")]
use actor_framework::{Receiver, TrySendError, embassy::Mailbox};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use std::{
    future::Future,
    pin::pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
};

#[derive(Default)]
struct WakeCount(AtomicUsize);
impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn backpressure_wakeup_and_drain() {
    let count = Arc::new(WakeCount::default());
    let waker = Waker::from(count.clone());
    let mut cx = Context::from_waker(&waker);
    let mut mailbox = Mailbox::<NoopRawMutex, _, 1>::new();
    let (mut tx, mut rx) = mailbox.split();
    tx.try_send(1).unwrap();
    assert_eq!(tx.try_send(2), Err(TrySendError::Full(2)));
    {
        let mut send = pin!(tx.send(2));
        assert!(send.as_mut().poll(&mut cx).is_pending());
        assert_eq!(pin!(rx.receive()).poll(&mut cx), Poll::Ready(Some(1)));
        assert!(count.0.load(Ordering::SeqCst) > 0);
        assert_eq!(send.as_mut().poll(&mut cx), Poll::Ready(Ok(())));
    }
    tx.close();
    assert_eq!(tx.try_send(3), Err(TrySendError::Closed(3)));
    assert_eq!(pin!(rx.receive()).poll(&mut cx), Poll::Ready(Some(2)));
    assert_eq!(pin!(rx.receive()).poll(&mut cx), Poll::Ready(None));
}

#[test]
fn cancellation_and_endpoint_drop() {
    let waker = Waker::noop();
    let mut cx = Context::from_waker(waker);
    let mut mailbox = Mailbox::<NoopRawMutex, _, 1>::new();
    let (mut tx, mut rx) = mailbox.split();
    assert!(pin!(rx.receive()).poll(&mut cx).is_pending());
    tx.try_send(1).unwrap();
    assert!(pin!(tx.send(2)).poll(&mut cx).is_pending()); // cancelled
    assert_eq!(pin!(rx.receive()).poll(&mut cx), Poll::Ready(Some(1)));
    assert!(pin!(rx.receive()).poll(&mut cx).is_pending());
    drop(tx);
    assert_eq!(pin!(rx.receive()).poll(&mut cx), Poll::Ready(None));
}

#[test]
fn zero_capacity_and_receiver_drop() {
    let mut mailbox = Mailbox::<NoopRawMutex, _, 0>::new();
    let (mut tx, rx) = mailbox.split();
    assert_eq!(tx.try_send(1), Err(TrySendError::Full(1)));
    let mut send = pin!(tx.send(2));
    let mut cx = Context::from_waker(Waker::noop());
    assert!(send.as_mut().poll(&mut cx).is_pending());
    drop(rx);
    assert_eq!(
        send.as_mut().poll(&mut cx),
        Poll::Ready(Err(actor_framework::SendError))
    );
}
