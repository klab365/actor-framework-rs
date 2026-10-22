#![cfg(feature = "tokio")]

use actor_framework::{actor, actor_handler, actor_system};

#[derive(actor_framework::ActorCommand)]
#[actor_command(response = Result<u32, ()>)]
struct Add(u32);
struct Changed;
struct Counter<T> {
    total: T,
}

impl<T> Counter<T>
where
    T: Copy + From<u32> + Into<u32>,
{
    #[actor_handler]
    async fn add(
        &mut self,
        events: actor_framework::ActorAddress<actor_framework::ActorEvent<Changed>>,
        message: Add,
    ) -> Result<u32, ()> {
        self.total = T::from(self.total.into() + message.0);
        events.publish(Changed).await.unwrap();
        Ok(self.total.into())
    }
}

actor! {
    actor generic_counter: Counter<u32> {
        mailbox_capacity: 1;
        commands { Add => Counter::<u32>::add; }
        events { Changed => []; }
    };
}
actor_system! { struct System { actors: [generic_counter]; } }

#[tokio::test]
async fn generic_inherent_impl_preserves_state_context_and_response() {
    let system = System::builder().generic_counter(Counter { total: 2 }).start().unwrap();
    assert_eq!(system.request(Add(3)).await, Ok(Ok(5)));
    assert_eq!(system.request(Add(4)).await, Ok(Ok(9)));
    system.shutdown().await.unwrap();
}
