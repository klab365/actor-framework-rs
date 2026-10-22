#![cfg(feature = "tokio")]

use actor_framework::{actor_module, actor_system};
#[derive(actor_framework::ActorCommand)]
#[actor_command(response = ())]
struct Amount(u32);
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Changed(u32);

#[derive(Debug, actor_framework::ActorCommand)]
#[actor_command(response = ())]
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
#[derive(actor_framework::ActorCommand)]
#[actor_command(response = ())]
struct Slow(u8);
#[derive(actor_framework::ActorCommand)]
#[actor_command(response = ())]
struct Crash(tokio::sync::oneshot::Sender<()>);
#[derive(actor_framework::ActorCommand)]
#[actor_command(response = ())]
struct BlockUntilReleased;
struct ShutdownActor;
#[derive(actor_framework::ActorCommand)]
#[actor_command(response = ())]
struct PartialClosed;
#[derive(Clone)]
struct PartialEvent;
#[derive(actor_framework::ActorCommand)]
#[actor_command(response = ())]
struct ClosePartial(tokio::sync::oneshot::Sender<()>);
#[derive(actor_framework::ActorCommand)]
#[actor_command(response = ())]
struct QueueMessage;

struct PartialRecorder;

struct QueueState;

static SLOW_STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static SLOW_RELEASE: tokio::sync::Notify = tokio::sync::Notify::const_new();
static SHUTDOWN_STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static SHUTDOWN_RELEASE: tokio::sync::Notify = tokio::sync::Notify::const_new();

#[actor_framework::actor_handler]
async fn increment(events: actor_framework::ActorAddress<actor_framework::ActorEvent<Unobserved>>, state: &mut Counter, amount: Amount) {
    state.value += amount.0;
    events.publish(Unobserved).await.unwrap();
}

#[actor_framework::actor_handler]
async fn record(state: &mut Recorder, event: Changed) {
    state.values.lock().await.push(event.0);
}

#[actor_framework::actor_handler]
async fn reset(_state: &mut ResetActor, _message: Reset) {
    RESET_SEEN.store(true, std::sync::atomic::Ordering::SeqCst);
}

#[actor_framework::actor_handler]
async fn wait_slow(_state: &mut SlowActor, _message: Slow) {
    SLOW_STARTED.store(true, std::sync::atomic::Ordering::SeqCst);
    SLOW_RELEASE.notified().await;
}

#[actor_framework::actor_handler]
async fn crash(_state: &mut CrashActor, _message: Crash) {
    let _ = _message.0.send(());
    panic!("intentional actor exit for closed-mailbox test");
}

#[actor_framework::actor_handler]
async fn block_until_released(_state: &mut ShutdownActor, _message: BlockUntilReleased) {
    SHUTDOWN_STARTED.store(true, std::sync::atomic::Ordering::SeqCst);
    SHUTDOWN_RELEASE.notified().await;
}

#[actor_framework::actor_handler]
async fn record_partial(_state: &mut PartialRecorder, event: PartialEvent) {
    PARTIAL_FIRST.lock().await.push(event);
    PARTIAL_FIRST_DELIVERED.notify_one();
}

#[actor_framework::actor_handler]
async fn close_partial(_state: &mut PartialClosed, message: ClosePartial) {
    let _ = message.0.send(());
    panic!("intentional actor exit for partial-publish test");
}

#[actor_framework::actor_handler]
async fn discard_partial(_state: &mut PartialClosed, _event: PartialEvent) {}

#[actor_framework::actor_handler]
async fn record_partial_last(_state: &mut PartialRecorder, event: PartialEvent) {
    PARTIAL_LAST.lock().await.push(event);
    PARTIAL_LAST_DELIVERED.notify_one();
}

#[actor_framework::actor_handler]
async fn queue_message(_state: &mut QueueState, _message: QueueMessage) {
    QUEUE_STARTED.notify_one();
    QUEUE_RELEASE.notified().await;
    QUEUE_COMPLETED.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

static FIRST: std::sync::LazyLock<Arc<Mutex<Vec<u32>>>> = std::sync::LazyLock::new(|| Arc::new(Mutex::new(Vec::new())));
static SECOND: std::sync::LazyLock<Arc<Mutex<Vec<u32>>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(Vec::new())));
static RESET_SEEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static PARTIAL_FIRST: std::sync::LazyLock<Arc<Mutex<Vec<PartialEvent>>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(Vec::new())));
static PARTIAL_LAST: std::sync::LazyLock<Arc<Mutex<Vec<PartialEvent>>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(Vec::new())));
static PARTIAL_FIRST_DELIVERED: tokio::sync::Notify = tokio::sync::Notify::const_new();
static PARTIAL_LAST_DELIVERED: tokio::sync::Notify = tokio::sync::Notify::const_new();
static QUEUE_STARTED: tokio::sync::Notify = tokio::sync::Notify::const_new();
static QUEUE_RELEASE: tokio::sync::Notify = tokio::sync::Notify::const_new();
static QUEUE_COMPLETED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

actor_module! {
    pub manifest counters;
    actors: [
        counter: Counter {
            mailbox_capacity: 2;
            commands { Amount => increment; }
            events { Unobserved => []; }
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
        },
        partial_first: PartialRecorder {
            mailbox_capacity: 1;
            commands {}
            events { PartialEvent => record_partial; }
        },
        partial_closed: PartialClosed {
            mailbox_capacity: 1;
            commands { ClosePartial => close_partial; }
            events { PartialEvent => discard_partial; }
        },
        partial_last: PartialRecorder {
            mailbox_capacity: 1;
            commands {}
            events { PartialEvent => record_partial_last; }
        },
        queue: QueueState {
            mailbox_capacity: 2;
            commands { QueueMessage => queue_message; }
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
        .partial_first(PartialRecorder)
        .partial_closed(PartialClosed)
        .partial_last(PartialRecorder)
        .queue(QueueState)
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
    actors.send(Amount(5)).await.unwrap();
    actors.publish(Changed(5)).await.unwrap();
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
    let actors = start_actors();
    let (started, received) = tokio::sync::oneshot::channel();
    actors.send(Crash(started)).await.unwrap();
    received.await.unwrap();
    for _ in 0..20 {
        let (started, _) = tokio::sync::oneshot::channel();
        if actors.send(Crash(started)).await.is_err() {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("mailbox should have closed after actor exit");
}

#[tokio::test]
async fn shutdown_reports_actor_panics() {
    let actors = start_actors();
    let (started, received) = tokio::sync::oneshot::channel();
    actors.send(Crash(started)).await.unwrap();
    received.await.unwrap();
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
async fn publish_stops_at_closed_subscriber_without_rolling_back_prior_delivery() {
    PARTIAL_FIRST.lock().await.clear();
    PARTIAL_LAST.lock().await.clear();
    let actors = start_actors();

    let (closed_tx, closed_rx) = tokio::sync::oneshot::channel();
    actors.send(ClosePartial(closed_tx)).await.unwrap();
    closed_rx.await.unwrap();
    let mut mailbox_closed = false;
    for _ in 0..20 {
        let (closed_tx, _) = tokio::sync::oneshot::channel();
        if actors.send(ClosePartial(closed_tx)).await.is_err() {
            mailbox_closed = true;
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(mailbox_closed, "closed subscriber mailbox should be unavailable");

    let first_delivery = PARTIAL_FIRST_DELIVERED.notified();
    tokio::pin!(first_delivery);
    assert!(actors.publish(PartialEvent).await.is_err());
    tokio::time::timeout(std::time::Duration::from_secs(1), &mut first_delivery)
        .await
        .expect("first subscriber should receive the event");
    assert_eq!(PARTIAL_FIRST.lock().await.len(), 1);
    assert!(PARTIAL_LAST.lock().await.is_empty());
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(10), PARTIAL_LAST_DELIVERED.notified())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn shutdown_discards_queued_messages_after_active_handler_completes() {
    QUEUE_COMPLETED.store(0, std::sync::atomic::Ordering::SeqCst);
    let actors = start_actors();

    let active_started = QUEUE_STARTED.notified();
    tokio::pin!(active_started);
    actors.send(QueueMessage).await.unwrap();
    active_started.await;
    actors.send(QueueMessage).await.unwrap();
    actors.send(QueueMessage).await.unwrap();

    let shutdown = actors.shutdown();
    tokio::pin!(shutdown);
    tokio::select! {
        biased;
        _ = &mut shutdown => panic!("shutdown returned before active handler completed"),
        _ = tokio::task::yield_now() => {}
    }
    assert!(actors.send(QueueMessage).await.is_err());
    QUEUE_RELEASE.notify_one();
    shutdown.await.unwrap();
    assert_eq!(QUEUE_COMPLETED.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn publishing_event_without_subscribers_is_successful_noop() {
    let actors = start_actors();
    actors.publish(Unobserved).await.unwrap();
}
