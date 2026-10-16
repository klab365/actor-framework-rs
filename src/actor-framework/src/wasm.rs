//! Allocating, local WebAssembly adapter for browser/worker JS event loops.
//!
//! Channels are bounded; tasks and pending sends still allocate/retain resources.
//! This module does not change the allocator-free core. Manifest macros select
//! this backend on wasm32 with the `wasm` feature.
//! No `Send` bound is imposed. See `docs/wasm.md` for lifecycle limitations.

use crate::{ActorPublishRoute, ActorSendRoute, SendError, TrySendError};

/// A zero capacity was requested; rendezvous channels are not supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidCapacity;

/// Failure before any actor task has been spawned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartError {
    /// Required builder state was omitted; contains the actor field name.
    MissingActorState(&'static str),
    /// An actor requested zero capacity; contains the actor field name.
    InvalidCapacity(&'static str),
}

/// Another shutdown call is already waiting. Retry after it completes or is cancelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShutdownInProgress;

/// Internal local-task executor used by generated WASM systems.
#[cfg(target_arch = "wasm32")]
#[doc(hidden)]
pub use wasm_bindgen_futures::spawn_local as __spawn_local;

/// Create a bounded FIFO channel. Capacity excludes the in-flight handler.
///
/// Zero returns an error. Storage allocates; capacity is never silently enlarged.
/// Dropping all senders closes the channel and permits draining. Dropping the
/// receiver rejects sends and drops queued messages. Pending operations wake on
/// closure. Neither handles nor messages are required to be `Send`.
pub fn channel<M>(capacity: usize) -> Result<(Sender<M>, Receiver<M>), InvalidCapacity> {
    if capacity == 0 {
        return Err(InvalidCapacity);
    }
    let (sender, receiver) = async_channel::bounded(capacity);
    Ok((Sender(sender), Receiver(receiver)))
}

/// A statically typed command route to one bounded mailbox.
///
/// Clone shares the same capacity, not a new queue. Awaiting a send acknowledges
/// acceptance only. A cancelled pending send drops its message without delivery.
/// Concurrent waiting senders have no adapter-level fairness guarantee.
/// Cyclic actor sends (including self-send) can deadlock under backpressure.
///
/// Unsupported message types are rejected at compile time:
/// ```compile_fail
/// use actor_framework::{ActorSendRoute, wasm};
/// let (sender, _receiver) = wasm::channel::<u32>(1).unwrap();
/// let _ = sender.send_route("not a u32");
/// ```
pub struct Sender<M>(async_channel::Sender<M>);

impl<M> Clone for Sender<M> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<M> Sender<M> {
    /// Reject new sends, wake waiters, and allow the receiver to drain.
    pub fn close(&self) {
        self.0.close();
    }

    /// Attempt acceptance without waiting, returning the message on failure.
    /// Closed takes precedence over full.
    pub fn try_send(&self, message: M) -> Result<(), TrySendError<M>> {
        self.0.try_send(message).map_err(|error| match error {
            async_channel::TrySendError::Full(message) => TrySendError::Full(message),
            async_channel::TrySendError::Closed(message) => TrySendError::Closed(message),
        })
    }
}

impl<M> ActorSendRoute<M> for Sender<M> {
    async fn send_route(&self, message: M) -> Result<(), SendError> {
        self.0.send(message).await.map_err(|_| SendError)
    }
}

/// Single consumer for a bounded mailbox; deliberately not cloneable.
/// Pending receive cancellation leaves queued messages untouched.
pub struct Receiver<M>(async_channel::Receiver<M>);

impl<M> crate::Receiver for Receiver<M> {
    type Message = M;

    async fn receive(&mut self) -> Option<M> {
        self.0.recv().await.ok()
    }
}

/// Statically sized, typed event fan-out in array order.
///
/// Each subscriber gets a clone. Zero subscribers succeeds. Delivery is
/// sequential and waits for each mailbox; closure stops at the first failure.
/// Failure or cancellation can leave earlier subscribers delivered, with no
/// rollback. Duplicate senders intentionally deliver multiple copies.
pub struct Publisher<M, const N: usize>(pub [Sender<M>; N]);

impl<M: Clone, const N: usize> ActorPublishRoute<M> for Publisher<M, N> {
    async fn publish_route(&self, message: M) -> Result<(), SendError> {
        for sender in &self.0 {
            sender.send_route(message.clone()).await?;
        }
        Ok(())
    }
}

/// Spawn an owned local actor on the WASM JS microtask executor.
///
/// Requires a wasm-bindgen JS host (browser, worker or Node), not WASI. Futures
/// may contain `Rc` and other non-`Send` values, but must own `'static` inputs.
/// Close/drop all senders to drain and finish. Context-held senders also count.
/// There is no join, abort, supervision or panic recovery; panics follow the
/// host's panic policy. Long CPU handlers must yield to avoid starving the host.
#[cfg(target_arch = "wasm32")]
pub fn spawn_actor<A, C, M>(mut actor: A, context: C, mut receiver: Receiver<M>)
where
    A: crate::Actor<M, C> + 'static,
    C: 'static,
    M: 'static,
{
    wasm_bindgen_futures::spawn_local(async move {
        crate::run(&mut actor, &context, &mut receiver).await;
    });
}
