#![cfg_attr(not(feature = "std"), no_std)]

//! Statically routed actors with bounded, runtime-specific mailboxes.
//!
//! The always-available [`core`] module requires neither an allocator nor an executor.
//! Manifest macros support the `tokio`, `wasm`, and `embassy` backends; lifecycle semantics
//! are runtime-specific.

pub mod core;

#[cfg(feature = "embassy")]
pub mod embassy;
#[cfg(feature = "wasm")]
pub mod wasm;
pub use crate::core::{
    Actor, ActorContext, ActorPublishRoute, ActorSendRoute, Mailbox, Receiver, SendError, TrySendError, run,
};
#[cfg(feature = "embassy")]
#[doc(hidden)]
pub use embassy_futures as __embassy_futures;
#[cfg(feature = "embassy")]
#[doc(hidden)]
pub use embassy_sync as __embassy_sync;

#[cfg(feature = "tokio")]
extern crate std;

#[cfg(feature = "tokio")]
mod tokio_runtime;

#[cfg(feature = "tokio")]
pub use tokio_runtime::{ActorTaskError, MissingActorState};

#[cfg(all(feature = "tokio", not(all(feature = "wasm", target_arch = "wasm32"))))]
#[doc(hidden)]
pub use actor_framework_macros::__actor_system;
#[cfg(all(feature = "embassy", not(feature = "tokio")))]
#[doc(hidden)]
pub use actor_framework_macros::__actor_system_embassy as __actor_system;
#[cfg(all(
    not(feature = "tokio"),
    not(feature = "embassy"),
    feature = "wasm",
    any(target_arch = "wasm32", not(feature = "tokio"))
))]
#[doc(hidden)]
pub use actor_framework_macros::__actor_system_wasm as __actor_system;
#[cfg(any(feature = "tokio", feature = "wasm", feature = "embassy"))]
pub use actor_framework_macros::actor_module;

/// Compose manifests using the selected runtime backend.
#[cfg(any(feature = "tokio", feature = "wasm", feature = "embassy"))]
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
#[cfg(any(feature = "tokio", feature = "wasm", feature = "embassy"))]
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
