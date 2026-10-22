use actor_framework::actor;

actor! {
    actors: [
        first: () { mailbox_capacity: 1; commands {} events {} },
        second: () { mailbox_capacity: 2; commands {} events {} }
    ];
}

fn main() {}
