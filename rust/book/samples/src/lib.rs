//! Code samples of the Edomata for Rust book (`rust/book`).
//!
//! Every chapter includes its code from here with mdBook's
//! `{{#include ...:anchor}}` directive, so the samples are compiled and
//! linted by `cargo clippy --workspace --all-targets`, and the pure ones are
//! exercised by `cargo test -p edomata-book-samples`. The samples that need
//! PostgreSQL or a broker are compiled but not run.

#![forbid(unsafe_code)]

pub mod cqrs;
pub mod eventsourcing;
pub mod migrations;
pub mod processes;
pub mod running;
pub mod saas;
pub mod simple;
pub mod testing;
