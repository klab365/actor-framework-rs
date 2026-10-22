struct WorkerState;
impl WorkerState {
#[actor_framework::actor_handler(request_capacity = 0)]
async fn invalid(&mut self, _: Command) {}
}

struct Command;
fn main() { let _: [(); WorkerState::__actor_handler_request_capacity_invalid] = []; }
