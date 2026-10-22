use actor_framework::{actor, actor_system};

struct Command;
struct State;

impl State {
#[actor_framework::actor_handler]
async fn first(&mut self, _: Command) {}
}
impl State {
#[actor_framework::actor_handler]
async fn second(&mut self, _: Command) {}
}

actor! {
    pub actor first_actor: State {
        mailbox_capacity: 1;
        commands { Command => State::first; }
        events {}
    };
}

actor! {
    pub actor second_actor: State {
        mailbox_capacity: 1;
        commands { Command => State::second; }
        events {}
    };
}

actor_system! {
    pub struct App {
        actors: [first_actor, second_actor];
    }
}

fn main() {}
