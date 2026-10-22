use actor_framework::actor;

actor! {
    actor first: () { mailbox_capacity: 1; commands {} events {} };
    actor second: () { mailbox_capacity: 2; commands {} events {} };
}

fn main() {}
