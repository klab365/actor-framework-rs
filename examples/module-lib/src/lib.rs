use actor_framework::actor;

#[derive(actor_framework::ActorCommand)]
#[actor_command(response = ())]
pub struct Add(pub u32);

#[derive(Clone)]
pub struct Added(pub u32);

pub struct Counter {
    pub total: u32,
}

impl Counter {
    #[actor_framework::actor_handler]
    pub async fn add(&mut self, changed: ActorAddress<ActorEvent<Added>>, message: Add) {
        println!("[module-lib/counter] received Add({})", message.0);
        self.total += message.0;
        LAST.store(self.total, std::sync::atomic::Ordering::SeqCst);
        changed.publish(Added(self.total)).await.expect("recorder route exists");
    }
}

pub struct Recorder;
pub static LAST: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

impl Recorder {
    #[actor_framework::actor_handler]
    pub async fn record(&mut self, event: Added) {
        println!("[module-lib/recorder] received Added({})", event.0);
        LAST.store(event.0, std::sync::atomic::Ordering::SeqCst);
    }
}

actor! {
    pub actor counter: Counter {
        mailbox_capacity: 8;
        commands { Add => Counter::add; }
        events {}
    };
}

actor! {
    pub actor recorder: Recorder {
        mailbox_capacity: 8;
        commands {}
        events { Added => Recorder::record; }
    };
}
