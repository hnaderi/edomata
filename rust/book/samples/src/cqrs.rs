//! Samples of the "CQRS style" chapter.

use edomata_core::*;
use serde::{Deserialize, Serialize};

// ANCHOR: result_nec
/// `EitherNec[R, A]` in Scala is `Result<A, NonEmpty<R>>` in Rust.
pub type ResultNec<A, R> = Result<A, NonEmpty<R>>;

pub fn either_examples() {
    let e1: ResultNec<i32, String> = Ok(1);
    let e2: ResultNec<&str, String> = Ok("Missile Launched!");
    let e3: ResultNec<i32, String> =
        Err(NonEmpty::new("No remained missiles to launch!".to_string()));

    let e4 = e1.map(|i| i * 2);
    let e5 = e2.and_then(|_| e4.clone()); // `>>`
    let e6 = e4.and_then(|a| e5.map(|b| a + b));
    assert_eq!(e6, Ok(4));
    assert!(e3.is_err());
}
// ANCHOR_END: result_nec

// ANCHOR: order
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    New,
    Cooking { cook: String },
    WaitingToPickUp,
    Delivering { unit: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rejection {
    ExistingOrder,
    NoSuchOrder,
    InvalidRequest, // more fine grained in real applications
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Order {
    Empty,
    Placed {
        food: String,
        address: String,
        status: OrderStatus,
    },
    Delivered {
        rating: i32,
    },
}

impl Order {
    pub fn place(&self, food: &str, address: &str) -> ResultNec<Order, Rejection> {
        match self {
            Order::Empty => Ok(Order::Placed {
                food: food.to_string(),
                address: address.to_string(),
                status: OrderStatus::New,
            }),
            _ => Err(NonEmpty::new(Rejection::ExistingOrder)),
        }
    }

    pub fn mark_as_cooking(&self, cook: &str) -> ResultNec<Order, Rejection> {
        match self {
            Order::Placed {
                food,
                address,
                status: OrderStatus::New,
            } => Ok(Order::Placed {
                food: food.clone(),
                address: address.clone(),
                status: OrderStatus::Cooking {
                    cook: cook.to_string(),
                },
            }),
            _ => Err(NonEmpty::new(Rejection::InvalidRequest)),
        }
    }

    // other logic from the business
}
// ANCHOR_END: order

// ANCHOR: model
/// A CQRS model only needs an initial state: there are no events to fold.
#[derive(Clone, Copy, Debug, Default)]
pub struct OrderModel;

impl CqrsModel for OrderModel {
    type State = Order;
    type Rejection = Rejection;

    fn initial(&self) -> Order {
        Order::Empty
    }
}
// ANCHOR_END: model

// ANCHOR: commands
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    Place { food: String, address: String },
    MarkAsCooking { cook: String },
    MarkAsCooked,
    MarkAsDelivering { unit: String },
    MarkAsDelivered,
    Rate { score: i32 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Notification {
    Received { food: String },
    Cooking,
    Cooked,
    Delivering,
    Delivered,
}
// ANCHOR_END: commands

// ANCHOR: service
/// The program type of our service: a `Stomaton` over a `CommandMessage`.
pub type OrderApp = CqrsApp<Command, Order, Rejection, Notification, ()>;

pub fn order_service() -> OrderApp {
    let dsl = OrderModel.dsl::<Command, Notification>();
    dsl.router(move |command| match command {
        Command::Place { food, address } => {
            let notification = Notification::Received { food: food.clone() };
            dsl.modify_s(move |order: Order| order.place(&food, &address))
                .void()
                .then(dsl.publish([notification]))
        }
        Command::MarkAsCooking { cook } => dsl
            .modify_s(move |order: Order| order.mark_as_cooking(&cook))
            .void()
            .then(dsl.publish([Notification::Cooking])),
        // other command handling logic
        _ => dsl.reject(Rejection::InvalidRequest),
    })
}
// ANCHOR_END: service

/// Running the service purely.
pub async fn run_scenario() -> ResponseE<Rejection, Notification, (Order, ())> {
    // ANCHOR: scenario
    let scenario1 = order_service()
        .run(
            CommandMessage::new(
                "cmd id",
                chrono::DateTime::<chrono::Utc>::MIN_UTC,
                "aggregate id",
                Command::Place {
                    food: "taco".to_string(),
                    address: "home".to_string(),
                },
            ),
            Order::Empty, // state to run the command on
        )
        .await;

    // `result` is the new state (or the rejections), `notifications` what to publish.
    let (new_state, ()) = scenario1.result.clone().expect("accepted");
    assert!(matches!(new_state, Order::Placed { .. }));
    assert_eq!(
        scenario1.notifications,
        vec![Notification::Received {
            food: "taco".to_string()
        }]
    );
    // ANCHOR_END: scenario
    scenario1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn either_examples_hold() {
        either_examples();
    }

    #[test]
    fn domain_model_is_pure() {
        // ANCHOR: model_tests
        assert!(Order::Empty.place("kebab", "home").is_ok());
        let pizza = Order::Empty.place("pizza", "office").unwrap();
        assert!(pizza.mark_as_cooking("chef").is_ok());
        // Can't place an order twice.
        assert_eq!(
            pizza.place("burger", "home"),
            Err(NonEmpty::new(Rejection::ExistingOrder))
        );
        // ANCHOR_END: model_tests
    }

    #[test]
    fn service_scenario() {
        futures::executor::block_on(run_scenario());
    }
}
