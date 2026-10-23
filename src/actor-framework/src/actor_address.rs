//! Statically routed, typed actor contexts.
//!
//! The `actor_handler` attribute lets a handler declare route capabilities as
//! `ActorAddress<ActorCommand<M>>`, `ActorAddress<ActorRequest<M>>`, or
//! `ActorAddress<ActorEvent<E>>`. The selected
//! system backend binds the concrete generated router type at composition time.
//! No dynamic registry or message-type lookup is used.

use core::marker::PhantomData;

use crate::{
    ActorPublishRoute, ActorRequestRoute, ActorSendRoute, ActorTrySendRoute, Command, RequestError, SendError,
    TrySendError,
};

/// Capability marker for sending one command message type.
pub struct ActorCommand<M>(PhantomData<fn() -> M>);

/// Capability marker for making one typed command request.
///
/// Unlike [`ActorCommand`], a request waits for the command handler's typed
/// response. Its bounded capacity and cancellation behavior are defined by the
/// selected runtime's generated [`ActorRequestRoute`].
pub struct ActorRequest<M>(PhantomData<fn() -> M>);

/// Capability marker for publishing one event message type.
pub struct ActorEvent<M>(PhantomData<fn() -> M>);

/// Typed actor-to-actor route capability.
///
/// This type is normally constructed by the generated handler adapter. Its
/// first generic argument declares one permitted operation and message type;
/// the concrete router type is inferred by that adapter.
pub struct ActorAddress<'a, C, Router> {
    router: &'a Router,
    _capability: PhantomData<fn() -> C>,
}

impl<'a, C, Router> ActorAddress<'a, C, Router> {
    /// Construct a typed address over a generated router.
    #[doc(hidden)]
    pub fn new(router: &'a Router) -> Self {
        Self {
            router,
            _capability: PhantomData,
        }
    }
}

impl<M, Router> ActorAddress<'_, ActorCommand<M>, Router>
where
    Router: ActorSendRoute<M>,
{
    /// Post a command to its unique actor, waiting for bounded mailbox capacity.
    /// Success acknowledges acceptance, not handling.
    pub async fn post(&self, message: M) -> Result<(), SendError> {
        self.router.send_route(message).await
    }

    /// Attempt to post without waiting; return the message if the mailbox is full.
    pub fn try_post(&self, message: M) -> Result<(), TrySendError<M>>
    where
        Router: ActorTrySendRoute<M>,
    {
        self.router.try_send_route(message)
    }
}

impl<M, Router> ActorAddress<'_, ActorRequest<M>, Router>
where
    M: Command,
    Router: ActorRequestRoute<M>,
{
    /// Send a command and wait for its typed handler response.
    ///
    /// `RequestError::Closed` means shutdown stopped the destination before
    /// acceptance; `ResponseDropped` means an accepted request lost its reply.
    /// Dropping this future cancels the caller's wait; runtime-specific reply
    /// resources are reclaimed according to the selected backend.
    pub async fn request(&self, message: M) -> Result<M::Response, RequestError> {
        self.router.request_route(message).await
    }
}

impl<E, Router> ActorAddress<'_, ActorEvent<E>, Router>
where
    Router: ActorPublishRoute<E>,
{
    /// Publish to the composed subscribers. A route with no subscribers succeeds.
    pub async fn publish(&self, event: E) -> Result<(), SendError> {
        self.router.publish_route(event).await
    }
}
