use actor_framework::{actor_handler, actor_module};

pub struct Add(pub u32);

#[derive(Clone)]
pub struct Added(pub u32);

pub struct Counter {
    pub total: u32,
}

#[actor_handler]
pub async fn add(state: &mut Counter, message: Add) {
    println!("[module-lib/counter] received Add({})", message.0);
    state.total += message.0;
    LAST.store(state.total, std::sync::atomic::Ordering::SeqCst);
}

pub struct Recorder;
pub static LAST: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[actor_handler]
pub async fn record(_state: &mut Recorder, event: Added) {
    println!("[module-lib/recorder] received Added({})", event.0);
    LAST.store(event.0, std::sync::atomic::Ordering::SeqCst);
}

actor_module! {
    pub manifest counter;
events: [Added];
    actors: [
        counter: Counter {
            mailbox_capacity: 8;
            commands { Add => add; }
            events {}
        },
        recorder: Recorder {
            mailbox_capacity: 8;
            commands {}
            events { Added => record; }
        }
    ];
}
