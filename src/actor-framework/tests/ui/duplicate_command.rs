use actor_framework::{actor_module, actor_system};

struct Command;
struct State;

#[actor_framework::actor_handler]
async fn first(_: &App, _: &mut State, _: Command) {}
#[actor_framework::actor_handler]
async fn second(_: &App, _: &mut State, _: Command) {}

actor_module! {
    pub actor first_actor: State {
        mailbox_capacity: 1;
        commands { Command => first; }
        events {}
    };
}

actor_module! {
    pub actor second_actor: State {
        mailbox_capacity: 1;
        commands { Command => second; }
        events {}
    };
}

actor_system! {
    pub struct App {
        actors: [first_actor, second_actor];
    }
}

fn main() {}
