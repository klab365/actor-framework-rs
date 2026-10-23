#![cfg(feature = "tokio")]
struct WorkerState;

#[derive(actor_framework::ActorCommand)]
#[actor_command(response = Result<u32, &'static str>)]
struct Query(bool);

#[derive(actor_framework::ActorCommand)]
#[actor_command(response = Result<(), ()>)]
struct CrashQuery;

#[derive(actor_framework::ActorCommand)]
#[actor_command(response = Result<u32, &'static str>)]
struct Forward(bool);

struct RouterState;

impl WorkerState {
    #[actor_framework::actor_handler]
    async fn handle(&mut self, query: Query) -> Result<u32, &'static str> {
        if query.0 { Ok(42) } else { Err("rejected") }
    }
}

impl WorkerState {
    #[actor_framework::actor_handler]
    async fn crash(&mut self, _: CrashQuery) -> Result<(), ()> {
        panic!("request actor panic")
    }
}

impl RouterState {
    #[actor_framework::actor_handler]
    async fn forward(
        &mut self,
        worker: actor_framework::ActorAddress<actor_framework::ActorRequest<Query>>,
        command: Forward,
    ) -> Result<u32, &'static str> {
        worker.request(Query(command.0)).await.unwrap()
    }
}

actor_framework::actor! {
    actor worker: WorkerState {
    mailbox_capacity: 2;
    commands { Query => WorkerState::handle;, CrashQuery => WorkerState::crash; }
    events {}
};
}
actor_framework::actor! {
    actor router: RouterState {
        mailbox_capacity: 2;
        commands { Forward => RouterState::forward; }
        events {}
    };
}
actor_framework::actor_system! { struct Queries { actors: [worker, router]; } }

#[tokio::test]
async fn request_separates_handler_result_from_transport_error() {
    let system = Queries::builder()
        .worker(WorkerState)
        .router(RouterState)
        .start()
        .unwrap();
    assert_eq!(system.request(Forward(true)).await, Ok(Ok(42)));
    assert_eq!(system.request(Forward(false)).await, Ok(Err("rejected")));
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
    let system = Queries::builder()
        .worker(WorkerState)
        .router(RouterState)
        .start()
        .unwrap();
    assert_eq!(
        system.request(CrashQuery).await,
        Err(actor_framework::RequestError::ResponseDropped)
    );
    assert!(system.shutdown().await.is_err());
}
