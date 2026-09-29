//! Code samples of the Edomata for Rust book (`rust/book`).
//!
//! Every chapter includes its code from here with mdBook's
//! `{{#include ...:anchor}}` directive, so the samples are compiled and
//! linted by `cargo clippy --workspace --all-targets --all-features`, and the pure ones are
//! exercised by `cargo test -p edomata-book-samples`, like the PostgreSQL
//! integration test of the testing chapter. The other samples that need
//! PostgreSQL or a broker are compiled but not run; the broker ones are
//! behind the `kafka` and `rabbitmq` features.

#![forbid(unsafe_code)]

pub mod brokers;
pub mod cookbook;
pub mod cqrs;
pub mod eventsourcing;
pub mod migrations;
pub mod operations;
pub mod processes;
pub mod running;
pub mod saas;
pub mod simple;
pub mod testing;
pub mod troubleshooting;
