use actor_framework::{ActorSendRoute, Receiver, actor_module, actor_system};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::prelude::*;

/// Number of messages received by each actor in one demonstration run.
const ROUNDS: u8 = 3;
const ROUND_DELAY_MS: i32 = 700;

#[derive(Debug)]
pub struct Ping(pub u8);

#[derive(Debug)]
pub struct Pong(pub u8);

pub struct PingActor(pub Rc<Cell<u8>>);

pub struct PongActor {
    received: Rc<Cell<u8>>,
    completed: actor_framework::wasm::Sender<()>,
}

#[actor_framework::actor_handler]
pub async fn receive_ping(pong: ActorAddress<ActorCommand<Pong>>, state: &mut PingActor, Ping(round): Ping) {
    pause_between_rounds().await;
    state.0.set(state.0.get() + 1);
    log(&format!("Ping received round {round}"));
    log(&format!("Ping sending Pong round {round}"));
    pong.post(Pong(round)).await.unwrap();
}

#[actor_framework::actor_handler]
pub async fn receive_pong(ping: ActorAddress<ActorCommand<Ping>>, state: &mut PongActor, Pong(round): Pong) {
    pause_between_rounds().await;
    state.received.set(state.received.get() + 1);
    log(&format!("Pong received round {round}"));

    if round == ROUNDS {
        log("Pong reached the final round and is terminating the sequence");
        log("Pong sending completion acknowledgement");
        ActorSendRoute::send_route(&state.completed, ()).await.unwrap();
    } else {
        let next_round = round + 1;
        log(&format!("Pong sending Ping round {next_round}"));
        ping.post(Ping(next_round)).await.unwrap();
    }
}

actor_module! {
    manifest ping_pong;
    events: [];
    actors: [
        ping: PingActor {
            mailbox_capacity: 1;
            commands { Ping => receive_ping; }
            events {}
        },
        pong: PongActor {
            mailbox_capacity: 1;
            commands { Pong => receive_pong; }
            events {}
        }
    ];
}

actor_system! {
    pub struct PingPongActors { modules: [ping_pong]; }
}

/// Run a finite browser-visible ping-pong exchange.
///
/// The returned JavaScript `Promise` resolves after Ping and Pong have each
/// received exactly [`ROUNDS`] messages. Call this after initializing the
/// wasm-bindgen module; `examples/wasm/index.html` does so on page load.
#[wasm_bindgen]
pub async fn run_ping_pong() {
    let ping_received = Rc::new(Cell::new(0));
    let pong_received = Rc::new(Cell::new(0));
    let (completed, mut completion) = actor_framework::wasm::channel(1).unwrap();
    let actors = PingPongActors::builder()
        .ping(PingActor(ping_received.clone()))
        .pong(PongActor {
            received: pong_received.clone(),
            completed,
        })
        .start()
        .unwrap();

    log("Starting Ping round 1");
    actors.send(Ping(1)).await.unwrap();
    assert_eq!(Receiver::receive(&mut completion).await, Some(()));
    actors.shutdown().await.unwrap();
    assert_eq!(ping_received.get(), ROUNDS);
    assert_eq!(pong_received.get(), ROUNDS);
    log("Ping-pong sequence completed");
}

async fn pause_between_rounds() {
    let window = web_sys::window().expect("the example requires a browser window");
    let delay = js_sys::Promise::new(&mut |resolve, _| {
        window
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ROUND_DELAY_MS)
            .expect("failed to schedule the ping-pong delay");
    });
    wasm_bindgen_futures::JsFuture::from(delay)
        .await
        .expect("the ping-pong delay promise should resolve");
}

fn log(message: &str) {
    web_sys::console::log_1(&message.into());
}
