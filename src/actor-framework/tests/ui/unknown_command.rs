use actor_framework::{actor_module, actor_system};

struct Known;
struct Unknown;
struct State;

async fn handle(_: &ActuatorContext, _: &mut State, _: Known) {}

actor_module! {
    pub manifest routes;
    events: [];
    actors: [
        actor: State {
            mailbox_capacity: 1;
            commands { Known => handle; }
            events {}
        }
    ];
}

actor_system! {
    pub struct App {
        modules: [routes];
    }
}

async fn invalid(app: &App) {
    let _ = app.send(Unknown).await;
}

fn main() {}
