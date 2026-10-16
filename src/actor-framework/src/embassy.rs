//! Heap-free Embassy mailbox adapter. See `docs/embassy.md` for task integration.
//!
//! One sender and one receiver are issued by exclusive [`Mailbox::split`].
//! Each endpoint has one waiter; neither endpoint is cloneable. Use a local
//! `NoopRawMutex` for one executor, or `CriticalSectionRawMutex` with a platform
//! critical-section implementation for interrupt/cross-executor access.

use crate::{Mailbox as Storage, SendError, TrySendError};
use core::{
    cell::RefCell,
    future::poll_fn,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    task::Poll,
};
use embassy_sync::{
    blocking_mutex::{
        Mutex,
        raw::{CriticalSectionRawMutex, RawMutex},
    },
    mutex::Mutex as AsyncMutex,
    signal::Signal,
    waitqueue::{MultiWakerRegistration, WakerRegistration},
};

/// Failure to initialize a generated Embassy actor system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartError {
    /// A required actor state was not supplied to the builder.
    MissingActorState(&'static str),
    /// A fixed Embassy task pool has no available slot.
    TaskCapacityExhausted,
    /// The system's statically allocated mailbox storage was already initialized.
    AlreadyStarted,
    /// A mailbox must have at least one slot so shutdown can enqueue its sentinel.
    InvalidCapacity(&'static str),
}

/// Static lifecycle state for generated Embassy actor systems.
pub struct Lifecycle {
    closed: AtomicBool,
    start_state: core::sync::atomic::AtomicU8,
    remaining: AtomicUsize,
    done: Signal<CriticalSectionRawMutex, ()>,
    shutdown: Signal<CriticalSectionRawMutex, ()>,
    wait_lock: AsyncMutex<CriticalSectionRawMutex, ()>,
    close_wakers: Mutex<CriticalSectionRawMutex, RefCell<MultiWakerRegistration<16>>>,
}

impl Lifecycle {
    /// Create lifecycle state for `workers` generated actor tasks.
    pub const fn new(workers: usize) -> Self {
        Self {
            closed: AtomicBool::new(false),
            start_state: core::sync::atomic::AtomicU8::new(0),
            remaining: AtomicUsize::new(workers),
            done: Signal::new(),
            shutdown: Signal::new(),
            wait_lock: AsyncMutex::new(()),
            close_wakers: Mutex::new(RefCell::new(MultiWakerRegistration::new())),
        }
    }
    /// Reserve the one-time start transition.
    pub fn begin_start(&self) -> core::result::Result<(), StartError> {
        self.start_state
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ())
            .map_err(|_| StartError::AlreadyStarted)
    }
    /// Release the start reservation when no tasks were spawned.
    pub fn abort_start(&self) {
        let _ = self
            .start_state
            .compare_exchange(1, 0, Ordering::AcqRel, Ordering::Acquire);
    }
    /// Mark a successfully reserved actor system as started.
    pub fn commit_start(&self) {
        self.start_state.store(2, Ordering::Release);
    }
    /// Whether new route sends are rejected.
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
    /// Close routes and notify the generated shutdown coordinator.
    /// Returns true only for the shutdown initiator.
    pub fn request_shutdown(&self) -> bool {
        if !self.closed.swap(true, Ordering::AcqRel) {
            self.close_wakers.lock(|cell| cell.borrow_mut().wake());
            self.shutdown.signal(());
            true
        } else {
            false
        }
    }
    /// Wait until shutdown has closed routes.
    pub async fn wait_closed(&self) {
        poll_fn(|cx| {
            if self.is_closed() {
                return Poll::Ready(());
            }
            self.close_wakers.lock(|cell| {
                let mut registrations = cell.borrow_mut();
                if self.is_closed() {
                    return Poll::Ready(());
                }
                registrations.register(cx.waker());
                if self.is_closed() {
                    Poll::Ready(())
                } else {
                    Poll::Pending
                }
            })
        })
        .await
    }
    /// Wait for the generated shutdown coordinator request.
    pub async fn shutdown_requested(&self) {
        self.shutdown.wait().await;
    }
    /// Mark one worker as stopped.
    pub fn complete(&self) {
        if self.remaining.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.done.signal(());
        }
    }
    /// Wait until all workers have stopped.
    pub async fn wait(&self) {
        let _guard = self.wait_lock.lock().await;
        while self.remaining.load(Ordering::Acquire) != 0 {
            self.done.wait().await;
        }
    }
}

struct State<M, const N: usize> {
    queue: Storage<M, N>,
    sender: WakerRegistration,
    receiver: WakerRegistration,
}

/// Inline bounded storage plus synchronization and two wake registrations.
/// Capacity excludes the active handler and the message held by a pending send.
/// Zero capacity is always full, not a rendezvous channel.
pub struct Mailbox<R: RawMutex, M, const N: usize> {
    state: Mutex<R, RefCell<State<M, N>>>,
}

impl<R: RawMutex, M, const N: usize> Default for Mailbox<R, M, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: RawMutex, M, const N: usize> Mailbox<R, M, N> {
    /// Construct empty, open, const-initializable storage without allocation.
    pub const fn new() -> Self {
        Self {
            state: Mutex::new(RefCell::new(State {
                queue: Storage::new(),
                sender: WakerRegistration::new(),
                receiver: WakerRegistration::new(),
            })),
        }
    }

    /// Borrow unique endpoints. Closure is permanent, including across splits.
    /// Dropping either endpoint closes the mailbox and wakes both waiters.
    pub fn split(&mut self) -> (Sender<'_, R, M, N>, Receiver<'_, R, M, N>) {
        (Sender { mailbox: self }, Receiver { mailbox: self })
    }

    fn close(&self) {
        self.state.lock(|cell| {
            let mut state = cell.borrow_mut();
            state.queue.close();
            state.sender.wake();
            state.receiver.wake();
        });
    }
}

/// Unique producer. Mutable operations prevent competing sender waiters.
pub struct Sender<'a, R: RawMutex, M, const N: usize> {
    mailbox: &'a Mailbox<R, M, N>,
}

impl<R: RawMutex, M, const N: usize> Sender<'_, R, M, N> {
    /// Reject new messages and wake waiters; accepted messages remain drainable.
    pub fn close(&mut self) {
        self.mailbox.close();
    }

    /// Never wait; return the original message on full or closed (closed wins).
    pub fn try_send(&mut self, message: M) -> Result<(), TrySendError<M>> {
        self.mailbox.state.lock(|cell| {
            let mut state = cell.borrow_mut();
            state.queue.try_send(message)?;
            state.receiver.wake();
            Ok(())
        })
    }

    /// Wait for capacity. Success acknowledges acceptance, not handling.
    /// Closure returns `SendError` and drops the message. Cancellation before
    /// acceptance drops it without enqueueing. No fairness guarantee is made.
    pub async fn send(&mut self, message: M) -> Result<(), SendError> {
        let mut message = Some(message);
        poll_fn(|cx| {
            self.mailbox.state.lock(|cell| {
                let mut state = cell.borrow_mut();
                match state.queue.try_send(message.take().expect("polled after completion")) {
                    Ok(()) => {
                        state.receiver.wake();
                        Poll::Ready(Ok(()))
                    }
                    Err(TrySendError::Closed(_)) => Poll::Ready(Err(SendError)),
                    Err(TrySendError::Full(value)) => {
                        message = Some(value);
                        state.sender.register(cx.waker());
                        Poll::Pending
                    }
                }
            })
        })
        .await
    }
}

impl<R: RawMutex, M, const N: usize> Drop for Sender<'_, R, M, N> {
    fn drop(&mut self) {
        self.mailbox.close();
    }
}

/// Unique consumer implementing the core receiver contract.
/// Drop closes but does not discard queued messages; storage retains ownership.
pub struct Receiver<'a, R: RawMutex, M, const N: usize> {
    mailbox: &'a Mailbox<R, M, N>,
}

impl<R: RawMutex, M, const N: usize> crate::Receiver for Receiver<'_, R, M, N> {
    type Message = M;

    async fn receive(&mut self) -> Option<M> {
        poll_fn(|cx| {
            self.mailbox.state.lock(|cell| {
                let mut state = cell.borrow_mut();
                if let Some(message) = state.queue.try_receive() {
                    state.sender.wake();
                    Poll::Ready(Some(message))
                } else if state.queue.is_closed() {
                    Poll::Ready(None)
                } else {
                    state.receiver.register(cx.waker());
                    Poll::Pending
                }
            })
        })
        .await
    }
}

impl<R: RawMutex, M, const N: usize> Drop for Receiver<'_, R, M, N> {
    fn drop(&mut self) {
        self.mailbox.close();
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::Lifecycle;

    #[test]
    fn shutdown_is_retryable_and_waiters_complete_after_all_workers() {
        let lifecycle = Lifecycle::new(2);
        lifecycle.begin_start().unwrap();
        lifecycle.commit_start();
        assert!(lifecycle.request_shutdown());
        assert!(!lifecycle.request_shutdown());
        use core::{
            future::Future,
            pin::pin,
            task::{Context, Poll, Waker},
        };
        {
            let mut wait = pin!(lifecycle.wait());
            let mut context = Context::from_waker(Waker::noop());
            lifecycle.complete();
            assert_eq!(wait.as_mut().poll(&mut context), Poll::Pending);
        }
        lifecycle.complete();
        futures_lite::future::block_on(lifecycle.wait());
    }

    #[test]
    fn close_wakes_pending_senders_and_concurrent_waiters_finish() {
        use core::{
            future::Future,
            pin::pin,
            task::{Context, Poll, Waker},
        };
        let lifecycle = Lifecycle::new(0);
        let mut closed = pin!(lifecycle.wait_closed());
        let mut context = Context::from_waker(Waker::noop());
        assert_eq!(closed.as_mut().poll(&mut context), Poll::Pending);
        lifecycle.request_shutdown();
        assert_eq!(closed.as_mut().poll(&mut context), Poll::Ready(()));
        futures_lite::future::block_on(async {
            futures_lite::future::zip(lifecycle.wait(), lifecycle.wait()).await;
        });
    }

    #[test]
    fn failed_start_reservation_can_be_retried() {
        let lifecycle = Lifecycle::new(1);
        lifecycle.begin_start().unwrap();
        lifecycle.abort_start();
        lifecycle.begin_start().unwrap();
        lifecycle.commit_start();
        assert!(lifecycle.begin_start().is_err());
    }
}
