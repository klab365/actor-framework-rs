/// Error returned when a required actor state was not provided to the builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingActorState(pub &'static str);

/// Identifies an actor task that panicked or exited without reporting completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActorTaskError {
    /// Manifest field name of the failed actor.
    pub actor: &'static str,
}
