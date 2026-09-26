//! Port of `examples/src/main/scala/Example1.scala`: an event-sourced
//! counter on PostgreSQL, the two styles of writing a service, and the
//! decision syntax helpers.
//!
//! ```sh
//! cargo run -p edomata-examples --bin counter
//! ```

use std::time::Duration;

use edomata_backend::RetryConfig;
use edomata_backend::eventsourcing::Backend;
use edomata_core::syntax::*;
use edomata_core::*;
use edomata_examples::{command, connect};
use edomata_sqlx::SqlxDriver;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Event {
    Opened,
    Received(i32),
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Rejection {
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Counter {
    Empty,
    Open(i32),
    Closed,
}

impl Counter {
    /// Receives `i`: opens the counter first when needed, refuses when
    /// closed.
    fn receive(&self, i: i32) -> Decision<Rejection, Event, Counter> {
        let decision = match self {
            Counter::Empty => Decision::accept(nonempty![Event::Opened, Event::Received(i)]),
            Counter::Open(_) => Decision::accept(Event::Received(i)),
            Counter::Closed => Decision::reject(Rejection::Unknown),
        };
        CounterModel.perform(self.clone(), decision)
    }
}

struct CounterModel;

impl DomainModel for CounterModel {
    type State = Counter;
    type Event = Event;
    type Rejection = Rejection;

    fn initial(&self) -> Counter {
        Counter::Empty
    }

    // The Scala example leaves the state untouched on every event; the
    // counter here actually counts, which is what a reader expects.
    fn transition(&self, event: &Event, state: Counter) -> Result<Counter, NonEmpty<Rejection>> {
        Ok(match (event, state) {
            (Event::Opened, _) => Counter::Open(0),
            (Event::Received(i), Counter::Open(n)) => Counter::Open(n + i),
            (Event::Received(i), _) => Counter::Open(*i),
            (Event::Closed, _) => Counter::Closed,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Updates {
    Updated,
    Closed,
}

type CounterApp = App<String, Counter, Event, Rejection, Updates, ()>;

/// `Application.app` in Scala: a router over string commands.
fn app() -> CounterApp {
    let dsl = CounterModel.dsl::<String, Updates>();
    dsl.router(move |cmd| match cmd.as_str() {
        "" => dsl.read().map(|ctx| ctx.command.derive_meta()).void(),
        "receive" => dsl
            .state()
            .and_then(move |s| dsl.decide(s.receive(2)))
            .and_then(move |ns| {
                dsl.eval(move || {
                    let ns = ns.clone();
                    async move { println!("new state: {ns:?}") }
                })
            })
            .and_then(move |()| dsl.publish([Updates::Updated])),
        _ => dsl.reject(Rejection::Unknown),
    })
}

/// `CounterService` in Scala: the same service written against the model's
/// decisions directly.
fn counter_service() -> CounterApp {
    let dsl = CounterModel.dsl::<String, Updates>();
    dsl.router(move |cmd| match cmd.as_str() {
        "" => dsl
            .state()
            .and_then(move |s| dsl.decide(s.receive(2).void())),
        _ => dsl.reject(Rejection::Unknown),
    })
}

/// `SyntaxExample` in Scala: the `edomata_core::syntax` helpers.
fn syntax_example() {
    let l1: Decision<Rejection, Event, ()> = Err::<(), _>(Rejection::Unknown).to_decision();
    let l2: Decision<Rejection, Event, ()> = Err::<Event, _>(Rejection::Unknown).to_accepted();
    let l4: Decision<Rejection, Event, ()> =
        Ok::<_, NonEmpty<Rejection>>(Event::Closed).to_accepted_nec();
    let l5: Decision<Rejection, Event, ()> = Some(Event::Closed).to_accepted();
    let l6: Decision<Rejection, Event, ()> = Some(Rejection::Unknown).to_rejected();
    let l7: Decision<Rejection, Event, ()> = None::<Event>.to_accepted_or(Rejection::Unknown);
    let l8: Decision<Rejection, Event, ()> = Event::Closed.accept();
    let l9: Decision<Rejection, Event, ()> = Rejection::Unknown.reject();
    let l10: Decision<Rejection, Event, i32> = 1.into_decision();
    println!("syntax: {l1:?} {l2:?} {l4:?} {l5:?} {l6:?} {l7:?} {l8:?} {l9:?} {l10:?}");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // `Counter.Empty.perform(Decision.accept(Event.Opened))` in Scala.
    let ns = CounterModel.perform(Counter::Empty, Decision::accept(Event::Opened));
    println!("performed: {ns:?}");
    syntax_example();

    let pool = connect().await?;
    let driver = SqlxDriver::for_namespace("counter", pool).await?;
    let backend = Backend::builder(CounterModel, CounterModel.dsl::<String, Updates>())
        .driver(driver)
        .in_mem_snapshot(200)
        .with_retry_config(RetryConfig {
            initial_delay: Duration::from_secs(2),
            ..RetryConfig::default()
        })
        .build_default()
        .await?;

    let service = backend.compile(app());
    let result = service(command("a", "receive".to_string())).await?;
    println!("receive: {result:?}");
    let result = backend.compile(counter_service())(command("a", String::new())).await?;
    println!("counter service: {result:?}");
    let state = backend.repository().get("a").await?;
    println!("state of a: {state:?}");
    backend.close().await?;
    Ok(())
}
