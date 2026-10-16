#![cfg_attr(not(feature = "std"), no_std)]

//! Statically routed actors with bounded, runtime-specific mailboxes.
//!
//! The always-available [`core`] module requires neither an allocator nor an executor.
//! Manifest macros support the `tokio` and `wasm` backends; lifecycle semantics
//! are runtime-specific.

pub mod core;

#[cfg(feature = "wasm")]
pub mod wasm;
pub use crate::core::{
    Actor, ActorContext, ActorPublishRoute, ActorSendRoute, Mailbox, Receiver, SendError, TrySendError, run,
};

#[cfg(feature = "tokio")]
extern crate std;

#[cfg(feature = "tokio")]
mod tokio_runtime;

#[cfg(feature = "tokio")]
pub use tokio_runtime::{ActorTaskError, MissingActorState};

#[cfg(all(feature = "tokio", not(all(feature = "wasm", target_arch = "wasm32"))))]
#[doc(hidden)]
pub use actor_framework_macros::__actor_system;
#[cfg(all(feature = "wasm", any(target_arch = "wasm32", not(feature = "tokio"))))]
#[doc(hidden)]
pub use actor_framework_macros::__actor_system_wasm as __actor_system;
#[cfg(any(feature = "tokio", feature = "wasm"))]
pub use actor_framework_macros::actor_module;

/// Compose manifests into a WASM system on wasm32 with `wasm`, otherwise Tokio.
#[cfg(any(feature = "tokio", feature = "wasm"))]
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
#[cfg(any(feature = "tokio", feature = "wasm"))]
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
