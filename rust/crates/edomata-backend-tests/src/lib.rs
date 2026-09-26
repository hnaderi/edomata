//! Shared behavioural suites for Edomata storages: the port of
//! `modules/backend-tests`.
//!
//! Every check is an `async fn` taking a backend (or a snapshot
//! persistence) so that the same checks run against the in-memory driver
//! and against PostgreSQL. Test crates call them from `#[tokio::test]`
//! functions, which replaces Scala's `StorageSuite.check`.

#![forbid(unsafe_code)]

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
