#[derive(actor_framework::ActorCommand)]
#[actor_command(response = Result<u32, ()>)]
struct Query;

#[actor_framework::actor_handler]
async fn handler(_: &mut (), _: Query) -> Result<bool, ()> { Ok(true) }

actor_framework::actor! {
    actor worker: () { mailbox_capacity: 1; commands { Query => handler; } events {} };
}
actor_framework::actor_system! { struct System { actors: [worker]; } }
fn main() {}
