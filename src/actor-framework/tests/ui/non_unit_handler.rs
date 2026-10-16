use actor_framework::{actor_module, actor_system};

struct Command;
struct State;

async fn handle(_context: &ActuatorContext, _state: &mut State, _message: Command) -> Result<(), ()> {
    Ok(())
}

actor_module! {
    pub manifest routes;
events: [];
    actors: [
        worker: State {
            mailbox_capacity: 1;
            commands { Command => handle; }
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
