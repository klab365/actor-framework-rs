use actor_framework::actor;
struct State;
struct Message;
actor! {
    actor state: State {
        mailbox_capacity: 1;
        commands { Message => handle; }
        events {}
    };
}
fn main() {}
