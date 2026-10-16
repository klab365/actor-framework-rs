#![cfg(feature = "tokio")]

use actor_framework::{ActorContext, actor_module, actor_system};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Changed(u32);

struct Reset;

struct Unobserved;

struct Counter {
    value: u32,
}

struct Recorder {
    values: Arc<Mutex<Vec<u32>>>,
}

struct ResetActor;

struct SlowActor;
struct CrashActor;
struct Slow(u8);
struct Crash;
struct BlockUntilReleased;
struct ShutdownActor;

static SLOW_STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static SLOW_RELEASE: tokio::sync::Notify = tokio::sync::Notify::const_new();
static CRASHED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static SHUTDOWN_STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static SHUTDOWN_RELEASE: tokio::sync::Notify = tokio::sync::Notify::const_new();

async fn increment(context: &impl counters::ActorContext, state: &mut Counter, amount: u32) {
    state.value += amount;
    context.publish(Changed(state.value)).await.unwrap();
    context.send(Reset).await.unwrap();
}

async fn record(_context: &impl ActorContext, state: &mut Recorder, event: Changed) {
    state.values.lock().await.push(event.0);
}

async fn reset(_context: &impl ActorContext, _state: &mut ResetActor, _message: Reset) {
    RESET_SEEN.store(true, std::sync::atomic::Ordering::SeqCst);
}

async fn wait_slow(_context: &impl ActorContext, _state: &mut SlowActor, _message: Slow) {
    SLOW_STARTED.store(true, std::sync::atomic::Ordering::SeqCst);
    SLOW_RELEASE.notified().await;
}

async fn crash(_context: &impl ActorContext, _state: &mut CrashActor, _message: Crash) {
    CRASHED.store(true, std::sync::atomic::Ordering::SeqCst);
    panic!("intentional actor exit for closed-mailbox test");
}

async fn block_until_released(_context: &impl ActorContext, _state: &mut ShutdownActor, _message: BlockUntilReleased) {
    SHUTDOWN_STARTED.store(true, std::sync::atomic::Ordering::SeqCst);
    SHUTDOWN_RELEASE.notified().await;
}

static FIRST: std::sync::LazyLock<Arc<Mutex<Vec<u32>>>> = std::sync::LazyLock::new(|| Arc::new(Mutex::new(Vec::new())));
static SECOND: std::sync::LazyLock<Arc<Mutex<Vec<u32>>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(Vec::new())));
static RESET_SEEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

actor_module! {
    pub manifest counters;
    context {
        send: [Reset];
        publish: [Changed];
    }
    events: [Changed, Unobserved];
    actors: [
        counter: Counter {
            mailbox_capacity: 2;
            commands { u32 => increment; }
            events {}
        },
        first_recorder: Recorder {
            mailbox_capacity: 2;
            commands {}
            events { Changed => record; }
        },
        second_recorder: Recorder {
            mailbox_capacity: 2;
            commands {}
            events { Changed => record; }
        }
    ];
}

actor_module! {
    pub manifest maintenance;
    context {}
    events: [];
    actors: [
        crash_actor: CrashActor {
            mailbox_capacity: 1;
            commands { Crash => crash; }
            events {}
        },
        slow_actor: SlowActor {
            mailbox_capacity: 1;
            commands { Slow => wait_slow; }
            events {}
        },
        reset_actor: ResetActor {
            mailbox_capacity: 2;
            commands { Reset => reset; }
            events {}
        },
        shutdown_actor: ShutdownActor {
            mailbox_capacity: 2;
            commands { BlockUntilReleased => block_until_released; }
            events {}
        }
    ];
}

actor_system! {
    pub struct RuntimeSystem {
        modules: [counters, maintenance];
    }
}

fn start_actors() -> RuntimeSystem {
    RuntimeSystem::builder()
        .counter(Counter { value: 0 })
        .first_recorder(Recorder { values: FIRST.clone() })
        .second_recorder(Recorder { values: SECOND.clone() })
        .crash_actor(CrashActor)
        .slow_actor(SlowActor)
        .reset_actor(ResetActor)
        .shutdown_actor(ShutdownActor)
        .start()
        .unwrap()
}

#[test]
fn builder_reports_missing_actor_state() {
    match RuntimeSystem::builder().counter(Counter { value: 0 }).start() {
        Err(error) => assert_eq!(error.0, "first_recorder"),
        Ok(_) => panic!("missing state should be rejected"),
    }
}

#[tokio::test]
async fn composes_multiple_modules_and_broadcasts_to_each_subscriber() {
    RESET_SEEN.store(false, std::sync::atomic::Ordering::SeqCst);
    FIRST.lock().await.clear();
    SECOND.lock().await.clear();

    let actors = start_actors();
    actors.send(5_u32).await.unwrap();
    actors.send(Reset).await.unwrap();

    for _ in 0..20 {
        if RESET_SEEN.load(std::sync::atomic::Ordering::SeqCst)
            && FIRST.lock().await.as_slice() == [5]
            && SECOND.lock().await.as_slice() == [5]
        {
            return;
        }
        tokio::task::yield_now().await;
    }

    assert!(RESET_SEEN.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(FIRST.lock().await.as_slice(), [5]);
    assert_eq!(SECOND.lock().await.as_slice(), [5]);
}

#[tokio::test]
async fn try_send_returns_the_message_when_mailbox_is_full() {
    SLOW_STARTED.store(false, std::sync::atomic::Ordering::SeqCst);
    let actors = start_actors();
    actors.send(Slow(1)).await.unwrap();
    for _ in 0..20 {
        if SLOW_STARTED.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(SLOW_STARTED.load(std::sync::atomic::Ordering::SeqCst));
    actors.send(Slow(2)).await.unwrap();
    assert!(matches!(
        actors.try_send(Slow(3)),
        Err(actor_framework::TrySendError::Full(Slow(3)))
    ));
    SLOW_RELEASE.notify_one();
}

#[tokio::test]
async fn sends_fail_after_actor_task_exits() {
    CRASHED.store(false, std::sync::atomic::Ordering::SeqCst);
    let actors = start_actors();
    actors.send(Crash).await.unwrap();
    for _ in 0..20 {
        if CRASHED.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(CRASHED.load(std::sync::atomic::Ordering::SeqCst));
    for _ in 0..20 {
        if actors.send(Crash).await.is_err() {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("mailbox should have closed after actor exit");
}

#[tokio::test]
async fn shutdown_reports_actor_panics() {
    CRASHED.store(false, std::sync::atomic::Ordering::SeqCst);
    let actors = start_actors();
    actors.send(Crash).await.unwrap();
    for _ in 0..20 {
        if CRASHED.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
        tokio::task::yield_now().await;
    }
    let error = actors.shutdown().await.unwrap_err();
    assert_eq!(error.actor, "crash_actor");
}

#[tokio::test]
async fn shutdown_waits_for_active_handler_and_rejects_new_messages() {
    SHUTDOWN_STARTED.store(false, std::sync::atomic::Ordering::SeqCst);
    let actors = start_actors();
    actors.send(BlockUntilReleased).await.unwrap();
    for _ in 0..20 {
        if SHUTDOWN_STARTED.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(SHUTDOWN_STARTED.load(std::sync::atomic::Ordering::SeqCst));

    let shutdown = actors.shutdown();
    tokio::pin!(shutdown);
    tokio::select! {
        _ = &mut shutdown => panic!("shutdown returned before active handler completed"),
        _ = tokio::time::sleep(std::time::Duration::from_millis(5)) => {}
    }
    SHUTDOWN_RELEASE.notify_one();
    shutdown.await.unwrap();
    assert!(actors.send(BlockUntilReleased).await.is_err());
}

#[tokio::test]
async fn publishing_event_without_subscribers_is_successful_noop() {
    let actors = start_actors();
    actors.publish(Unobserved).await.unwrap();
}
