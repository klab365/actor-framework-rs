use actor_framework::{ActorAddress, ActorEvent, actor_handler, actor_module, actor_system};

struct Command;
#[derive(Clone)]
struct Event;
struct State;

#[actor_handler]
async fn handle(events: ActorAddress<ActorEvent<Event>>, _state: &mut State, _message: Command) {
    events.post(Event).await.unwrap();
}

actor_module! {
    pub actor actor: State {
        mailbox_capacity: 1;
        commands { Command => handle; }
        events {}
    };
}

actor_system! {
    pub struct App { actors: [actor]; }
}

fn main() {}
