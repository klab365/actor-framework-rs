//! Heap-free contracts and caller-owned storage, with no runtime dependencies.
//!
//! Futures are concrete, may borrow their inputs and need not be `Send` or
//! `'static`. Executors pin and poll them in caller-provided storage. This module
//! neither spawns tasks nor provides synchronization or a dynamic registry.

/// Error returned when a message cannot be sent because its actor has stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendError;

/// Statically generated route for a permitted command send.
#[allow(async_fn_in_trait)]
pub trait ActorSendRoute<M> {
    /// Wait for bounded mailbox capacity; fail if the destination is closed.
    ///
    /// Success means acceptance, not completed handling. Dropping a pending send
    /// cancels that attempt; adapters must document any stronger guarantees.
    async fn send_route(&self, message: M) -> Result<(), SendError>;
}

/// Statically generated route for a permitted event publication.
#[allow(async_fn_in_trait)]
pub trait ActorPublishRoute<M> {
    /// Deliver to subscribers with bounded backpressure; no subscribers is success.
    ///
    /// Failure may follow partial delivery; there is no rollback. Adapters define
    /// subscriber order and cancellation behavior.
    async fn publish_route(&self, message: M) -> Result<(), SendError>;
}

/// Error returned by a non-blocking send.
#[derive(Debug, PartialEq, Eq)]
pub enum TrySendError<T> {
    /// The bounded mailbox has no available capacity.
    Full(T),
    /// The actor has stopped; the original message is returned.
    Closed(T),
}

/// Exclusively owned actor state handling one message at a time.
///
/// `M` is an ordinary owned value (possibly an adapter-generated enum).
/// No marker trait, allocation, `Send` or `'static` bound is required.
#[allow(async_fn_in_trait)]
pub trait Actor<M, C: ?Sized> {
    /// Handle a message to completion before the next message is dispatched.
    async fn handle(&mut self, context: &C, message: M);
}

/// Adapter boundary for a single-consumer bounded mailbox.
///
/// Implementations own or borrow their storage and synchronization. They must
/// document capacity and wake pending receivers when messages arrive or the
/// channel closes. A pending receive must not remove a message when cancelled.
#[allow(async_fn_in_trait)]
pub trait Receiver {
    /// Owned message accepted by this receiver.
    type Message;

    /// Wait for the next message; `None` means closed and drained, permanently.
    async fn receive(&mut self) -> Option<Self::Message>;
}

/// Process messages serially until the receiver is closed and drained.
///
/// The returned future borrows all storage; the executor must pin and poll it
/// inline or in statically reserved task storage. No spawn or allocation occurs.
/// Dropping it cancels the current handler and drops its in-flight message;
/// queued messages remain the receiver's responsibility. Tokio's generated
/// loops retain their distinct shutdown policy (discard queued messages).
pub async fn run<A, C: ?Sized, R>(actor: &mut A, context: &C, receiver: &mut R)
where
    R: Receiver,
    A: Actor<R::Message, C>,
{
    while let Some(message) = receiver.receive().await {
        actor.handle(context, message).await;
    }
}

/// Caller-owned FIFO storage containing at most `N` messages inline.
///
/// This is storage, not a synchronized channel: operations require exclusive
/// access and never wait or register wakers. An executor adapter must supply
/// synchronization and wakeups before exposing it through [`Receiver`].
/// `N = 0` is valid and always full (not a rendezvous channel).
/// Dropping the mailbox drops all queued messages.
pub struct Mailbox<M, const N: usize> {
    slots: [Option<M>; N],
    head: usize,
    len: usize,
    closed: bool,
}

impl<M, const N: usize> Default for Mailbox<M, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M, const N: usize> Mailbox<M, N> {
    /// Create empty storage, usable in a `static` initializer without allocation.
    pub const fn new() -> Self {
        Self {
            slots: [const { None }; N],
            head: 0,
            len: 0,
            closed: false,
        }
    }

    /// Maximum number of queued messages (excludes any in-flight handler).
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Number of queued messages.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether there are no queued messages.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether new sends are permanently rejected.
    pub const fn is_closed(&self) -> bool {
        self.closed
    }

    /// Reject future sends while preserving queued messages for draining.
    pub fn close(&mut self) {
        self.closed = true;
    }

    /// Enqueue without waiting; return ownership on full or closed storage.
    /// Closed takes precedence over full. Never overwrites a queued message.
    pub fn try_send(&mut self, message: M) -> Result<(), TrySendError<M>> {
        if self.closed {
            return Err(TrySendError::Closed(message));
        }
        if self.len == N {
            return Err(TrySendError::Full(message));
        }
        let tail = (self.head + self.len) % N;
        self.slots[tail] = Some(message);
        self.len += 1;
        Ok(())
    }

    /// Remove the oldest message, including after closure.
    /// `None` means empty, not necessarily closed; this operation never waits.
    pub fn try_receive(&mut self) -> Option<M> {
        if self.len == 0 {
            return None;
        }
        let message = self.slots[self.head].take();
        self.head = (self.head + 1) % N;
        self.len -= 1;
        message
    }
}
