//! Port of `examples/src/main/scala/StomatonExample.scala`: a CQRS
//! (state-only) aggregate, run purely and then through a PostgreSQL backend
//! with a transactional notification handler.
//!
//! ```sh
//! cargo run -p edomata-examples --bin stomaton
//! ```

use edomata_backend::BackendError;
use edomata_backend::cqrs::Backend;
use edomata_core::*;
use edomata_examples::{command, connect};
use edomata_sqlx::{SqlxCqrsDriver, SqlxHandler};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Foo {
    Empty,
    Started(i32),
}

impl Foo {
    fn start(&self, initial: i32) -> Result<Foo, NonEmpty<String>> {
        match self {
            Foo::Empty => Ok(Foo::Started(initial)),
            Foo::Started(_) => Err(NonEmpty::new("Cannot start started foo!".to_string())),
        }
    }
}

struct FooModel;

impl CqrsModel for FooModel {
    type State = Foo;
    type Rejection = String;

    fn initial(&self) -> Foo {
        Foo::Empty
    }
}

type FooApp<T> = CqrsApp<i32, Foo, String, i32, T>;

/// `FooService.apply()`: does nothing.
fn apply() -> FooApp<()> {
    FooModel.dsl::<i32, i32>().unit()
}

/// `FooService.apply2()`: starts the foo with 1 and returns the new state.
fn apply2() -> FooApp<Foo> {
    let dsl = FooModel.dsl::<i32, i32>();
    dsl.pure(1)
        .and_then(move |_| dsl.decide_s(|s: Foo| s.start(1)))
        .and_then(move |ns| dsl.unit().replace(ns))
}

/// A routed service: starts the foo with the command value and publishes it.
fn service() -> FooApp<()> {
    let dsl = FooModel.dsl::<i32, i32>();
    dsl.router(move |i| {
        dsl.decide_s(move |s: Foo| s.start(i))
            .void()
            .and_then(move |()| dsl.publish([i]))
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Running a stomaton purely.
    let res = apply2().run(command("foo-1", 0), Foo::Empty).await;
    match &res.result {
        Ok((new_state, _)) => println!("pure run: new state {new_state:?}"),
        Err(errs) => println!("pure run: rejected {errs:?}"),
    }
    let res = apply().run(command("foo-1", 0), Foo::Started(3)).await;
    println!(
        "unit app keeps the state: {:?}",
        res.result.map(|(s, ())| s)
    );

    // The notification handler runs inside the save transaction
    // (`SkunkHandler` in Scala).
    let handler: SqlxHandler<i32> = SqlxHandler::new(|ns, _conn| {
        Box::pin(async move {
            for i in ns.iter() {
                if *i < 5 {
                    println!("handler: {i}");
                } else {
                    println!("handler: {i} is higher than 5");
                }
            }
            Ok::<(), BackendError>(())
        })
    });

    let pool = connect().await?;
    let driver = SqlxCqrsDriver::for_namespace("stomaton_example", pool).await?;
    let backend = Backend::builder(FooModel, FooModel.dsl::<i32, i32>())
        .driver(driver)
        .with_event_handler(handler)
        .build_default()
        .await?;
    let srv = backend.compile(service());
    let address = uuid::Uuid::new_v4().to_string();
    println!("start with 3: {:?}", srv(command(&address, 3)).await?);
    println!("start with 7: {:?}", srv(command(&address, 7)).await?);
    println!("state: {:?}", backend.repository().get(&address).await?);
    Ok(())
}
