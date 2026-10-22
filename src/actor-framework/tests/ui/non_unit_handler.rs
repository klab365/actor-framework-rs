use actor_framework::{actor, actor_system};

struct Command;
struct State;

impl State {
#[actor_framework::actor_handler]
async fn handle(&mut self, _message: Command) -> Result<(), ()> {
    Ok(())
}
}

actor! {
    pub actor worker: State {
        mailbox_capacity: 1;
        commands { Command => State::handle; }
        events {}
    };
}

actor_system! {
    pub struct App {
        actors: [worker];
    }
}

fn main() {}
