use actor_framework::{actor, actor_system};

struct Known;
struct Unknown;
struct State;

impl State {
#[actor_framework::actor_handler]
async fn handle(&mut self, _: Known) {}
}

actor! {
    pub actor actor: State {
        mailbox_capacity: 1;
        commands { Known => State::handle; }
        events {}
    };
}

actor_system! {
    pub struct App {
        actors: [actor];
    }
}

async fn invalid(app: &App) {
    let _ = app.send(Unknown).await;
}

fn main() {}
