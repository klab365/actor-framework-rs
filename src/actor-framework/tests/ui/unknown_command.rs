use actor_framework::{actor_module, actor_system};

struct Known;
struct Unknown;
struct State;

#[actor_framework::actor_handler]
async fn handle(_: &mut State, _: Known) {}

actor_module! {
    pub actor actor: State {
        mailbox_capacity: 1;
        commands { Known => handle; }
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
