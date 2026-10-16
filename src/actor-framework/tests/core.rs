use actor_framework::{Actor, Mailbox, Receiver, TrySendError, run};
use std::{
    cell::Cell,
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
};

const EMPTY: Mailbox<u8, 2> = Mailbox::new();

#[test]
fn bounded_fifo_wraparound_and_close() {
    let mut mailbox = EMPTY;
    assert_eq!(mailbox.capacity(), 2);
    assert!(mailbox.is_empty());
    mailbox.try_send(1).unwrap();
    mailbox.try_send(2).unwrap();
    assert_eq!(mailbox.try_send(3), Err(TrySendError::Full(3)));
    assert_eq!(mailbox.try_receive(), Some(1));
    mailbox.try_send(3).unwrap();
    mailbox.close();
    assert!(mailbox.is_closed());
    assert_eq!(mailbox.try_send(4), Err(TrySendError::Closed(4)));
    assert_eq!(mailbox.try_receive(), Some(2));
    assert_eq!(mailbox.try_receive(), Some(3));
    assert_eq!(mailbox.try_receive(), None);
    assert_eq!(mailbox.len(), 0);
}

#[test]
fn zero_capacity_never_accepts() {
    let mut mailbox = Mailbox::<u8, 0>::new();
    assert_eq!(mailbox.try_send(1), Err(TrySendError::Full(1)));
    assert_eq!(mailbox.try_receive(), None);
    mailbox.close();
    assert_eq!(mailbox.try_send(2), Err(TrySendError::Closed(2)));
}

#[test]
fn queued_values_are_dropped_once() {
    struct Message<'a>(&'a Cell<usize>);
    impl Drop for Message<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let drops = Cell::new(0);
    let mut mailbox = Mailbox::<_, 2>::new();
    assert!(mailbox.try_send(Message(&drops)).is_ok());
    assert!(mailbox.try_send(Message(&drops)).is_ok());
    drop(mailbox.try_receive());
    assert_eq!(drops.get(), 1);
    drop(mailbox);
    assert_eq!(drops.get(), 2);
}

// A closed, prefilled receiver needs no synchronization or wakeup machinery.
struct ClosedReceiver(Mailbox<u8, 2>);
impl Receiver for ClosedReceiver {
    type Message = u8;
    async fn receive(&mut self) -> Option<u8> {
        self.0.try_receive()
    }
}

#[test]
fn borrowed_non_send_actor_runs_in_inline_future_storage() {
    struct LocalActor<'a>(&'a Cell<u8>);
    impl Actor<u8, Cell<u8>> for LocalActor<'_> {
        async fn handle(&mut self, context: &Cell<u8>, message: u8) {
            self.0.set(self.0.get() + message);
            context.set(context.get() + 1);
        }
    }
    let sum = Cell::new(0);
    let count = Cell::new(0);
    let mut actor = LocalActor(&sum);
    let mut receiver = ClosedReceiver(Mailbox::new());
    receiver.0.try_send(3).unwrap();
    receiver.0.try_send(4).unwrap();
    receiver.0.close();
    let mut future = pin!(run(&mut actor, &count, &mut receiver));
    let mut context = Context::from_waker(Waker::noop());
    assert_eq!(future.as_mut().poll(&mut context), Poll::Ready(()));
    assert_eq!(sum.get(), 7);
    assert_eq!(count.get(), 2);
}
