use actor_framework::{ActorAddress, ActorEvent, actor_handler, actor, actor_system};

struct Command;
#[derive(Clone)]
struct Event;
struct State;

impl State {
#[actor_handler]
async fn handle(&mut self, events: ActorAddress<ActorEvent<Event>>,  _message: Command) {
    events.post(Event).await.unwrap();
}
}

actor! {
    pub actor actor: State {
        mailbox_capacity: 1;
        commands { Command => State::handle; }
        events {}
    };
}

actor_system! {
    pub struct App { actors: [actor]; }
}

fn main() {}
