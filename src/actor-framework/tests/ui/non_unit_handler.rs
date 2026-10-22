use actor_framework::{actor_module, actor_system};

struct Command;
struct State;

#[actor_framework::actor_handler]
async fn handle(_state: &mut State, _message: Command) -> Result<(), ()> {
    Ok(())
}

actor_module! {
    pub actor worker: State {
        mailbox_capacity: 1;
        commands { Command => handle; }
        events {}
    };
}

actor_system! {
    pub struct App {
        actors: [worker];
    }
}

fn main() {}
