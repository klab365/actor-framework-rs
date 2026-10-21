#[actor_framework::actor_handler(request_capacity = 0)]
async fn invalid(_: &mut (), _: Command) {}

struct Command;
fn main() {}
