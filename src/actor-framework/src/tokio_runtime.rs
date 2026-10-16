/// Error returned when a required actor state was not provided to the builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingActorState(pub &'static str);

/// Identifies an actor task that panicked or exited without reporting completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActorTaskError {
    /// Manifest field name of the failed actor.
    pub actor: &'static str,
}

/// Error returned when a message cannot be sent because its actor has stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendError;

/// Statically generated route for a permitted command send.
#[allow(async_fn_in_trait)]
pub trait ActorSendRoute<M> {
    /// Send a message through this actor-system route.
    async fn send_route(&self, message: M) -> Result<(), SendError>;
}

/// Statically generated route for a permitted event publication.
#[allow(async_fn_in_trait)]
pub trait ActorPublishRoute<M> {
    /// Publish a message through this actor-system route.
    async fn publish_route(&self, message: M) -> Result<(), SendError>;
}

/// Context capability available to actor handlers.
#[allow(async_fn_in_trait)]
pub trait ActorContext {
    /// Send a command when the module context grants that route.
    async fn send<M>(&self, message: M) -> Result<(), SendError>
    where
        Self: ActorSendRoute<M>,
    {
        ActorSendRoute::send_route(self, message).await
    }

    /// Publish an event when the module context grants that route.
    async fn publish<M>(&self, message: M) -> Result<(), SendError>
    where
        Self: ActorPublishRoute<M>,
    {
        ActorPublishRoute::publish_route(self, message).await
    }
}

impl<T> ActorContext for T {}

/// Error returned by a non-blocking send.
#[derive(Debug, PartialEq, Eq)]
pub enum TrySendError<T> {
    /// The bounded mailbox has no available capacity.
    Full(T),
    /// The actor has stopped; the original message is returned.
    Closed(T),
}
