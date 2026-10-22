#![cfg(feature = "tokio")]

#[derive(actor_framework::ActorCommand)]
#[actor_command(response = Result<u32, &'static str>)]
struct Query(bool);

#[derive(actor_framework::ActorCommand)]
#[actor_command(response = Result<(), ()>)]
struct CrashQuery;

#[actor_framework::actor_handler]
async fn handle(_: &mut (), query: Query) -> Result<u32, &'static str> {
    if query.0 { Ok(42) } else { Err("rejected") }
}

#[actor_framework::actor_handler]
async fn crash(_: &mut (), _: CrashQuery) -> Result<(), ()> {
    panic!("request actor panic")
}

actor_framework::actor_module! {
    actor worker: () {
    mailbox_capacity: 2;
    commands { Query => handle;, CrashQuery => crash; }
    events {}
};
}
actor_framework::actor_system! { struct Queries { actors: [worker]; } }

#[tokio::test]
async fn request_separates_handler_result_from_transport_error() {
    let system = Queries::builder().worker(()).start().unwrap();
    assert_eq!(system.request(Query(true)).await, Ok(Ok(42)));
    assert_eq!(system.request(Query(false)).await, Ok(Err("rejected")));
    system.shutdown().await.unwrap();
    assert_eq!(
        system.request(Query(true)).await,
        Err(actor_framework::RequestError::Closed)
    );
}

#[tokio::test]
async fn actor_panic_drops_the_request_reply() {
    let system = Queries::builder().worker(()).start().unwrap();
    assert_eq!(
        system.request(CrashQuery).await,
        Err(actor_framework::RequestError::ResponseDropped)
    );
    assert!(system.shutdown().await.is_err());
}
