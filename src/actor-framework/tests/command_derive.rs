#![cfg(feature = "tokio")]

#[derive(actor_framework::ActorCommand)]
#[actor_command(response = Result<u32, CounterError>)]
struct GetCount;

#[derive(Debug, PartialEq)]
struct CounterError;

#[test]
fn derive_declares_the_response_type() {
    fn response_of<C: actor_framework::Command>() -> core::marker::PhantomData<C::Response> {
        core::marker::PhantomData
    }
    let _: core::marker::PhantomData<Result<u32, CounterError>> = response_of::<GetCount>();
}
