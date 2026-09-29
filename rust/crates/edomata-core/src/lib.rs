//! # Edomata core
//!
//! Core abstractions of [Edomata](https://github.com/beyond-scale-group/edomata),
//! a lightweight library for building event-driven automata: event-sourced
//! aggregates ([`Edomaton`]) and CQRS state machines ([`Stomaton`]).
//!
//! This crate is pure and runtime-agnostic: it has no dependency on any
//! async runtime and compiles to `wasm32-unknown-unknown`. Programs are
//! `Send` futures, so they can be driven by any executor (Tokio in the
//! PostgreSQL backends).
//!
//! | Type | Purpose |
//! |------|---------|
//! | [`Decision`] | State machine with `Accepted` / `Rejected` / `InDecisive` outcomes |
//! | [`ResponseD`] | A decision combined with notifications |
//! | [`ResponseE`] | A `Result` combined with notifications |
//! | [`Edomaton`] | Event-driven automaton (full event sourcing) |
//! | [`Stomaton`] | State-only automaton (CQRS without event sourcing) |
//! | [`DecisionT`] | Asynchronous decision |
//! | [`Action`] | Asynchronous response |
//! | [`DomainModel`] | Initial state and event transitions of an aggregate |
//! | [`DomainCompiler`] | Folds a program's decision into the model |
//!
//! ```
//! use edomata_core::*;
//!
//! // A counter that only accepts positive increments.
//! struct Counter;
//!
//! impl DomainModel for Counter {
//!     type State = i32;
//!     type Event = i32;
//!     type Rejection = String;
//!
//!     fn initial(&self) -> i32 { 0 }
//!
//!     fn transition(&self, event: &i32, state: i32) -> Result<i32, NonEmpty<String>> {
//!         if *event > 0 { Ok(state + event) } else { Err(NonEmpty::new("negative".into())) }
//!     }
//! }
//!
//! let dsl = Counter.dsl::<i32, String>();
//! let app = dsl.router(move |by| {
//!     if by == 0 { dsl.reject("zero".to_string()) } else { dsl.accept(by).publish(["changed".to_string()]) }
//! });
//!
//! let ctx = CommandMessage::new("cmd-1", chrono::DateTime::UNIX_EPOCH, "counter-1", 5).build_context(10);
//! let outcome = futures::executor::block_on(app.execute(&Counter, ctx));
//! assert_eq!(outcome, EdomatonResult::Accepted {
//!     new_state: 15,
//!     events: nonempty![5],
//!     notifications: vec!["changed".to_string()],
//! });
//! ```
//!
//! ## Where it fits
//!
//! `edomata-core` is the pure base of the workspace: it depends on no other
//! Edomata crate, and every other crate builds on it, directly or (for
//! `edomata-serde`, through `edomata-backend`) indirectly, except
//! `edomata-postgres`, which is independent of it.
//!
//! - `edomata-backend` runs [`Edomaton`] and [`Stomaton`] programs against a
//!   storage (journal, snapshots, outbox, optimistic concurrency, retries)
//!   and ships an in-memory driver;
//! - `edomata-postgres`, `edomata-serde` and `edomata-sqlx` provide the
//!   PostgreSQL naming and DDL, the payload codecs and the sqlx driver;
//! - `edomata-testkit` has assertion helpers for unit-testing programs;
//! - `edomata-saas` / `edomata-saas-sqlx` add multi-tenancy,
//!   `edomata-simple` a closure-based facade, and `edomata-broker`,
//!   `edomata-kafka` and `edomata-rabbitmq` distribute notifications.
//!
//! Domain code usually only needs this crate: define a [`DomainModel`] (or a
//! [`CqrsModel`]), write programs with its [`DomainDsl`] (or
//! [`CqrsDomainDsl`]), and unit-test them with [`Edomaton::execute`].
//!
//! ## Feature flags
//!
//! | Feature | Default | Effect |
//! |---------|---------|--------|
//! | `serde` | no | Derives `serde::Serialize` / `serde::Deserialize` for [`CommandMessage`], [`MessageMetadata`] and [`NonEmpty`] (and enables `chrono/serde`). Deserializing an empty sequence into a [`NonEmpty`] fails with [`EmptyError`]. |

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links, rustdoc::private_intra_doc_links)]
// `doc_auto_cfg` was merged into `doc_cfg` (Rust 1.92), which now shows
// feature-gated items on docs.rs automatically.
#![cfg_attr(docsrs, feature(doc_cfg))]

use std::future::Future;
use std::pin::Pin;

mod action;
mod compiler;
mod decision;
mod decision_t;
mod dsl;
mod edomaton;
mod model;
mod nonempty;
mod request_context;
mod response;
mod stomaton;
pub mod syntax;

pub use action::Action;
pub use compiler::{DomainCompiler, EdomatonResult};
pub use decision::Decision;
pub use decision_t::DecisionT;
pub use dsl::{App, CqrsApp, CqrsDomainDsl, DomainDsl};
pub use edomaton::Edomaton;
pub use model::{CqrsModel, DomainModel};
pub use nonempty::{EmptyError, NonEmpty};
pub use request_context::{CommandMessage, MessageMetadata, RequestContext};
pub use response::{RaiseError, ResponseD, ResponseE, ResponseT};
pub use stomaton::Stomaton;

/// A boxed, `Send` future, the shape of every effect in Edomata programs.
///
/// [`Edomaton::run`], [`Stomaton::run`], [`Action::run`] and
/// [`DecisionT::run`] all return this type.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A result carrying one or more rejections, the counterpart of Cats'
/// `EitherNec`.
pub type ResultNec<T, R> = Result<T, NonEmpty<R>>;

/// The type of a domain service: handles a command and returns unit or the
/// rejection reasons.
///
/// Backends compile an [`Edomaton`] (or a [`Stomaton`]) into a function of
/// this shape, which hides storage, concurrency and retries from callers.
pub type DomainService<'a, C, R> = dyn Fn(C) -> BoxFuture<'a, ResultNec<(), R>> + Send + Sync + 'a;
