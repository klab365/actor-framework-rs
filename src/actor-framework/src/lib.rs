#![cfg_attr(not(feature = "std"), no_std)]

//! Statically routed actors with bounded, runtime-specific mailboxes.
//!
//! The Tokio adapter is the first prototype. The runtime-neutral, heap-free core
//! and Embassy/WASM adapters are not implemented yet.

#[cfg(feature = "tokio")]
extern crate std;

#[cfg(feature = "tokio")]
mod tokio_runtime;

#[cfg(feature = "tokio")]
pub use tokio_runtime::{
    ActorContext, ActorPublishRoute, ActorSendRoute, ActorTaskError, MissingActorState, SendError, TrySendError,
};

#[doc(hidden)]
pub use actor_framework_macros::__actor_system;
pub use actor_framework_macros::actor_module;

/// Compose explicit actor module manifests into an actor system.
#[macro_export]
macro_rules! actor_system {
    (
        $vis:vis struct $name:ident {
            modules: [$($module:ident),* $(,)?];
        }
    ) => {
        $crate::__actor_framework_collect_modules! {
            $vis struct $name;
            actors: [];
            events: [];
            remaining: [$($module,)*];
        }
    };
}

/// Internal continuation used by [`actor_system!`].
#[doc(hidden)]
#[macro_export]
macro_rules! __actor_framework_collect_modules {
    (
        $vis:vis struct $name:ident;
        actors: [$($actors:tt)*];
        events: [$($events:ty,)*];
        remaining: [];
    ) => {
        $crate::__actor_system! {
            $vis struct $name {
                actors: [$($actors)*];
                events: [$($events,)*];
            }
        }
    };
    (
        $vis:vis struct $name:ident;
        actors: [$($actors:tt)*];
        events: [$($events:ty,)*];
        remaining: [$next:ident, $($remaining:ident,)*];
    ) => {
        $next! {
            $crate::__actor_framework_collect_modules,
            {
                $vis struct $name;
                actors: [$($actors)*];
                events: [$($events,)*];
                remaining: [$($remaining,)*];
            }
        }
    };
}
