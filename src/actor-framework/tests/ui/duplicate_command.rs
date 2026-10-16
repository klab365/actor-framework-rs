use actor_framework::{actor_module, actor_system};

struct Command;
struct State;

async fn first(_: &App, _: &mut State, _: Command) {}
async fn second(_: &App, _: &mut State, _: Command) {}

actor_module! {
    pub manifest routes;
    context {}
    events: [];
    actors: [
        first_actor: State {
            mailbox_capacity: 1;
            commands { Command => first; }
            events {}
        },
        second_actor: State {
            mailbox_capacity: 1;
            commands { Command => second; }
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
