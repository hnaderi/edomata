//! Shared behavioural suites for Edomata storages: the port of
//! `modules/backend-tests`.
//!
//! Every check is an `async fn` taking a backend (or a snapshot
//! persistence) so that the same checks run against the in-memory driver
//! and against PostgreSQL. Test crates call them from `#[tokio::test]`
//! functions, which replaces Scala's `StorageSuite.check`.
//!
//! ```
//! use edomata_backend::cqrs::Backend as CqrsBackend;
//! use edomata_backend::eventsourcing::Backend;
//! use edomata_backend::inmemory::InMemoryDriver;
//! use edomata_backend_tests::{cqrs, eventsourcing as es};
//! use edomata_backend_tests::{TestCqrsModel, TestDomain, test_cqrs_dsl, test_domain_dsl};
//!
//! # tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap().block_on(async {
//! // In a test crate, each check is usually its own `#[tokio::test]`.
//! let backend = Backend::builder(TestDomain, test_domain_dsl())
//!     .driver(InMemoryDriver::new())
//!     .build_default()
//!     .await
//!     .unwrap();
//! es::must_append_correctly(&backend).await;
//! es::appending_must_be_idempotent(&backend).await;
//!
//! let backend = CqrsBackend::builder(TestCqrsModel, test_cqrs_dsl())
//!     .driver(InMemoryDriver::new())
//!     .build_default()
//!     .await
//!     .unwrap();
//! cqrs::inserts_state(&backend).await;
//! # });
//! ```
//!
//! To check a new storage driver, build its backends with [`TestDomain`] /
//! [`TestCqrsModel`] and run every check of [`eventsourcing`] and [`cqrs`];
//! for the [`eventsourcing::prepared_data`] checks, seed the storage with
//! the rows that module describes first. `edomata-sqlx` does this in
//! `tests/shared_suites.rs`.
//!
//! ## Panics
//!
//! Every check panics (through `assert!` and `unwrap`) when the storage does
//! not behave as expected: that is how it fails the calling test.
//!
//! ## Where it fits
//!
//! A test-support crate: it depends on `edomata-core` and `edomata-backend`
//! and is a dev-dependency of the storage drivers (`edomata-sqlx`). It is
//! the port of the Scala `backend-tests` module. Its checks run against the
//! in-memory driver in this crate's own tests.
//!
//! ## Feature flags
//!
//! This crate has no Cargo feature flags.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links, rustdoc::private_intra_doc_links)]
// `doc_auto_cfg` was merged into `doc_cfg` (Rust 1.92), which now shows
// feature-gated items on docs.rs automatically.
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod cqrs;
pub mod eventsourcing;

use edomata_core::{CqrsDomainDsl, CqrsModel, DomainDsl, DomainModel, NonEmpty};

/// Port of `TestDomain`: an event-sourced counter whose events are added to
/// the state.
#[derive(Clone, Copy, Debug, Default)]
pub struct TestDomain;

impl DomainModel for TestDomain {
    type State = i32;
    type Event = i32;
    type Rejection = String;

    fn initial(&self) -> i32 {
        0
    }

    fn transition(&self, event: &i32, state: i32) -> Result<i32, NonEmpty<String>> {
        Ok(event + state)
    }
}

/// The DSL of [`TestDomain`] with `i32` commands and notifications.
pub fn test_domain_dsl() -> DomainDsl<i32, i32, i32, String, i32> {
    TestDomain.dsl()
}

/// Port of `TestCQRSModel`: an `i32` state.
#[derive(Clone, Copy, Debug, Default)]
pub struct TestCqrsModel;

impl CqrsModel for TestCqrsModel {
    type State = i32;
    type Rejection = String;

    fn initial(&self) -> i32 {
        0
    }
}

/// The DSL of [`TestCqrsModel`] with `i32` commands and notifications.
pub fn test_cqrs_dsl() -> CqrsDomainDsl<i32, i32, String, i32> {
    TestCqrsModel.dsl()
}

/// A random string (a UUID), the port of `Utils.randomString`.
pub fn random_string() -> String {
    uuid::Uuid::new_v4().to_string()
}
