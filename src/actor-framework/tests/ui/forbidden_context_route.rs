use actor_framework::{actor_module, actor_system};

struct Allowed;
struct Forbidden;
struct State;

async fn handle(context: &ActuatorContext, _state: &mut State, _message: Allowed) {
    context.send(Forbidden).await.unwrap();
}

actor_module! {
    pub manifest routes;
    events: [];
    actors: [
        worker: State {
            mailbox_capacity: 1;
            commands { Allowed => handle; }
            events {}
        }
    ];
}

actor_system! {
    pub struct App {
        modules: [routes];
    }
}

fn main() {}
