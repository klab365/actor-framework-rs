use actor_framework::actor_handler;
struct State;
struct Message;

#[actor_handler]
async fn free_function(_: &mut State, _: Message) {}

impl State {
    #[actor_handler]
    async fn shared(&self, _: Message) {}
    #[actor_handler]
    async fn owned(self, _: Message) {}
    #[actor_handler]
    async fn associated(_: Message) {}
    #[actor_handler]
    fn synchronous(&mut self, _: Message) {}
    #[actor_handler]
    async fn generic<T>(&mut self, _: T) {}
    #[actor_handler]
    async fn extra(&mut self, _: Message, _: Message) {}
}

trait Handler {
    #[actor_handler]
    async fn required(&mut self, _: Message);
}
trait DefaultHandler {
    #[actor_handler]
    async fn default_handler(&mut self, _: Message) {}
}
trait Implementation { async fn implemented(&mut self, _: Message); }
impl Implementation for State {
    #[actor_handler]
    async fn implemented(&mut self, _: Message) {}
}
fn main() {}
