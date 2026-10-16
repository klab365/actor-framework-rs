use actor_framework::actor_system;
use actor_framework_module_example::counter;
use actor_framework_module_example::{Add, Counter, LAST, Recorder};

actor_system! {
    pub struct AppActors {
        modules: [counter];
    }
}

#[tokio::main]
async fn main() {
    println!("[module-app] composing and starting the library actor module");
    let actors = AppActors::builder()
        .counter(Counter { total: 0 })
        .recorder(Recorder)
        .start()
        .expect("counter state supplied");
    println!("[module-app] sending Add(5) to the imported module");
    actors.send(Add(5)).await.expect("counter actor is running");
    for _ in 0..20 {
        if LAST.load(std::sync::atomic::Ordering::SeqCst) == 5 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    let observed = LAST.load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(observed, 5);
    println!("[module-app] imported module published and recorded Added({observed})");
}
