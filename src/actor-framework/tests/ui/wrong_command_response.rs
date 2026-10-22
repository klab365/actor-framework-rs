struct WorkerState;
#[derive(actor_framework::ActorCommand)]
#[actor_command(response = Result<u32, ()>)]
struct Query;

impl WorkerState {
#[actor_framework::actor_handler]
async fn handler(&mut self, _: Query) -> Result<bool, ()> { Ok(true) }
}

actor_framework::actor! {
    actor worker: WorkerState { mailbox_capacity: 1; commands { Query => WorkerState::handler; } events {} };
}
actor_framework::actor_system! { struct System { actors: [worker]; } }
fn main() {}
