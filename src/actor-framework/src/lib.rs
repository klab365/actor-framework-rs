#![cfg_attr(not(feature = "std"), no_std)]

//! Statically routed actors with bounded, runtime-specific mailboxes.
//!
//! An [`actor!`] declaration defines one actor and its bounded mailbox;
//! [`actor_system!`] composes declarations into typed command and event routes.
//! Route success acknowledges mailbox acceptance, not completed handling. The
//! always-available [`core`] module requires neither an allocator nor an executor.
//! Manifest macros support the `tokio`, `wasm`, and `embassy` backends; capacity,
//! shutdown, and storage semantics are runtime-specific.
//!
//! # Task memory is not mailbox capacity
//!
//! `mailbox_capacity` bounds queued envelopes, not stacks or future storage.
//! Tokio tasks use heap-backed futures and runtime worker-thread stacks (configured
//! with `tokio::runtime::Builder::thread_stack_size`). WASM local tasks have no
//! per-actor stack setting; toolchain and host memory limits apply. Embassy sizes
//! static future storage at compile time: one task pool slot per actor plus one
//! shutdown-coordinator slot per system, in addition to mailboxes, lifecycle state
//! and per-command reply pools. Executor/interrupt stacks remain separate.
//! There is no universal actor `stack_size` option.
//!
//! See the [resource planning guide](https://github.com/klab365/actor-framework-rs/blob/main/docs/task-resources.md)
//! for target RAM measurements, examples, reply storage and failure boundaries.

pub mod core;

pub mod actor_address;
#[cfg(feature = "embassy")]
pub mod embassy;
pub use actor_address::{ActorAddress, ActorCommand, ActorEvent, ActorRequest};
#[cfg(feature = "wasm")]
pub mod wasm;
pub use crate::core::{
    Actor, ActorPublishRoute, ActorRequestRoute, ActorSendRoute, ActorTrySendRoute, Command, Mailbox,
    ObserveCommandResponse, Receiver, RequestError, SendError, TrySendError, run,
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
pub use actor_framework_macros::{ActorCommand, actor, actor_handler};

/// Compose actors using the selected runtime backend.
#[cfg(any(feature = "tokio", feature = "wasm", feature = "embassy"))]
#[macro_export]
macro_rules! actor_system {
    (
        $vis:vis struct $name:ident {
            actors: [$($actor:ident), * $(, )?];
        }
    ) => {
        $crate::__actor_framework_collect_modules! {
            $vis struct $name;
            actors: [];
            events: [];
            remaining: [$($actor,)*];
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
