//! # Edomata testkit
//!
//! Assertion helpers for testing domain programs, the port of
//! `edomata-munit`'s `DomainSuite`. They run an [`Edomaton`] (or a
//! [`Stomaton`]) with a command and a state, and assert on the outcome with
//! plain panics, so they work with `#[test]`, `#[tokio::test]` or any
//! executor.
//!
//! ```
//! use edomata_core::*;
//! use edomata_testkit::EdomatonAssertions;
//!
//! struct Counter;
//! impl DomainModel for Counter {
//!     type State = i32; type Event = i32; type Rejection = String;
//!     fn initial(&self) -> i32 { 0 }
//!     fn transition(&self, e: &i32, s: i32) -> Result<i32, NonEmpty<String>> { Ok(s + e) }
//! }
//!
//! let dsl = Counter.dsl::<i32, String>();
//! let app = dsl.router(move |by| {
//!     if by == 0 { dsl.reject("zero".to_string()) } else { dsl.accept(by).publish(["changed".to_string()]) }
//! });
//!
//! futures::executor::block_on(async {
//!     app.expect(&Counter, 5, 10, 15, ["changed".to_string()]).await;
//!     app.expect_rejection_with(&Counter, 0, 10, ["zero".to_string()]).await;
//! });
//! ```

#![forbid(unsafe_code)]

use std::fmt::Debug;

use chrono::{DateTime, Utc};
use edomata_core::{
    CommandMessage, DomainModel, Edomaton, EdomatonResult, NonEmpty, RequestContext, ResponseE,
    Stomaton,
};

/// How test commands are built: message id, timestamp and aggregate address.
/// `Default` uses id `"1"`, the minimum timestamp (Scala's `Instant.MIN`,
/// here `DateTime::<Utc>::MIN_UTC`) and address `"sut"`, like Scala's
/// `DomainSuite`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestCommand {
    /// Message id of the generated commands.
    pub msg_id: String,
    /// Timestamp of the generated commands.
    pub time: DateTime<Utc>,
    /// Aggregate address of the generated commands.
    pub address: String,
}

impl Default for TestCommand {
    fn default() -> Self {
        Self {
            msg_id: "1".to_string(),
            time: DateTime::<Utc>::MIN_UTC,
            address: "sut".to_string(),
        }
    }
}

impl TestCommand {
    /// Custom message id and address.
    pub fn new(msg_id: impl Into<String>, address: impl Into<String>) -> Self {
        Self {
            msg_id: msg_id.into(),
            address: address.into(),
            ..Self::default()
        }
    }

    /// Wraps a payload in a command message.
    pub fn message<C>(&self, payload: C) -> CommandMessage<C> {
        CommandMessage::new(
            self.msg_id.clone(),
            self.time,
            self.address.clone(),
            payload,
        )
    }
}

/// Assertion helpers for event-sourced programs.
#[allow(async_fn_in_trait)]
pub trait EdomatonAssertions<C, S, E, R, N> {
    /// Runs the program against `state` with `command` (default
    /// [`TestCommand`]) and folds the decision into `model`.
    async fn run_with<M>(&self, model: &M, command: C, state: S) -> EdomatonResult<S, E, R, N>
    where
        M: DomainModel<State = S, Event = E, Rejection = R> + Sync;

    /// Like [`run_with`](Self::run_with) with a custom [`TestCommand`].
    async fn run_with_command<M>(
        &self,
        model: &M,
        test: &TestCommand,
        command: C,
        state: S,
    ) -> EdomatonResult<S, E, R, N>
    where
        M: DomainModel<State = S, Event = E, Rejection = R> + Sync;

    /// Asserts that the program accepts and yields exactly `expected_state`
    /// and `expected_notifications`, in order.
    async fn expect<M>(
        &self,
        model: &M,
        command: C,
        state: S,
        expected_state: S,
        expected_notifications: impl IntoIterator<Item = N>,
    ) where
        M: DomainModel<State = S, Event = E, Rejection = R> + Sync,
        S: PartialEq + Debug,
        E: Debug,
        R: Debug,
        N: PartialEq + Debug;

    /// Asserts exactly `expected_state`, and the same notifications in any
    /// order.
    async fn expect_all<M>(
        &self,
        model: &M,
        command: C,
        state: S,
        expected_state: S,
        expected_notifications: impl IntoIterator<Item = N>,
    ) where
        M: DomainModel<State = S, Event = E, Rejection = R> + Sync,
        S: PartialEq + Debug,
        E: Debug,
        R: Debug,
        N: Ord + Debug;

    /// Asserts that the program rejects, returning the notifications and
    /// reasons.
    async fn expect_rejection<M>(&self, model: &M, command: C, state: S) -> (Vec<N>, NonEmpty<R>)
    where
        M: DomainModel<State = S, Event = E, Rejection = R> + Sync,
        S: Debug,
        E: Debug,
        R: Debug,
        N: Debug;

    /// Asserts that the program rejects with exactly `expected_errors` and
    /// no notifications.
    async fn expect_rejection_with<M>(
        &self,
        model: &M,
        command: C,
        state: S,
        expected_errors: impl IntoIterator<Item = R>,
    ) where
        M: DomainModel<State = S, Event = E, Rejection = R> + Sync,
        S: Debug,
        E: Debug,
        R: PartialEq + Debug,
        N: PartialEq + Debug;

    /// Asserts that the program rejects with exactly `expected_errors` and
    /// `expected_notifications`.
    async fn expect_rejection_and_notify<M>(
        &self,
        model: &M,
        command: C,
        state: S,
        expected_errors: impl IntoIterator<Item = R>,
        expected_notifications: impl IntoIterator<Item = N>,
    ) where
        M: DomainModel<State = S, Event = E, Rejection = R> + Sync,
        S: Debug,
        E: Debug,
        R: PartialEq + Debug,
        N: PartialEq + Debug;

    /// Asserts that the program rejects and publishes exactly `notifications`.
    async fn expect_rejection_notify<M>(
        &self,
        model: &M,
        command: C,
        state: S,
        notifications: impl IntoIterator<Item = N>,
    ) where
        M: DomainModel<State = S, Event = E, Rejection = R> + Sync,
        S: Debug,
        E: Debug,
        R: Debug,
        N: PartialEq + Debug;

    /// Asserts that the program accepts with exactly `expected_notifications`
    /// and that `that` holds for the new state.
    async fn expect_that<M>(
        &self,
        model: &M,
        command: C,
        state: S,
        expected_notifications: impl IntoIterator<Item = N>,
        that: impl FnOnce(&S),
    ) where
        M: DomainModel<State = S, Event = E, Rejection = R> + Sync,
        S: Debug,
        E: Debug,
        R: Debug,
        N: PartialEq + Debug;
}

fn collect<T>(items: impl IntoIterator<Item = T>) -> Vec<T> {
    items.into_iter().collect()
}

impl<C, S, E, R, N, T> EdomatonAssertions<C, S, E, R, N>
    for Edomaton<RequestContext<C, S>, R, E, N, T>
where
    C: Send + 'static,
    S: Clone + Send + 'static,
    E: Send + 'static,
    R: Send + 'static,
    N: Send + 'static,
    T: Send + 'static,
{
    async fn run_with<M>(&self, model: &M, command: C, state: S) -> EdomatonResult<S, E, R, N>
    where
        M: DomainModel<State = S, Event = E, Rejection = R> + Sync,
    {
        self.run_with_command(model, &TestCommand::default(), command, state)
            .await
    }

    async fn run_with_command<M>(
        &self,
        model: &M,
        test: &TestCommand,
        command: C,
        state: S,
    ) -> EdomatonResult<S, E, R, N>
    where
        M: DomainModel<State = S, Event = E, Rejection = R> + Sync,
    {
        self.execute(model, RequestContext::new(test.message(command), state))
            .await
    }

    async fn expect<M>(
        &self,
        model: &M,
        command: C,
        state: S,
        expected_state: S,
        expected_notifications: impl IntoIterator<Item = N>,
    ) where
        M: DomainModel<State = S, Event = E, Rejection = R> + Sync,
        S: PartialEq + Debug,
        E: Debug,
        R: Debug,
        N: PartialEq + Debug,
    {
        match self.run_with(model, command, state).await {
            EdomatonResult::Accepted {
                new_state,
                notifications,
                ..
            } => {
                assert_eq!(new_state, expected_state, "unexpected state");
                assert_eq!(
                    notifications,
                    collect(expected_notifications),
                    "unexpected notifications"
                );
            }
            other => panic!("Expected success, but obtained: {other:?}"),
        }
    }

    async fn expect_all<M>(
        &self,
        model: &M,
        command: C,
        state: S,
        expected_state: S,
        expected_notifications: impl IntoIterator<Item = N>,
    ) where
        M: DomainModel<State = S, Event = E, Rejection = R> + Sync,
        S: PartialEq + Debug,
        E: Debug,
        R: Debug,
        N: Ord + Debug,
    {
        match self.run_with(model, command, state).await {
            EdomatonResult::Accepted {
                new_state,
                mut notifications,
                ..
            } => {
                assert_eq!(new_state, expected_state, "unexpected state");
                let mut expected = collect(expected_notifications);
                notifications.sort();
                expected.sort();
                assert_eq!(notifications, expected, "unexpected notifications");
            }
            other => panic!("Expected success, but obtained: {other:?}"),
        }
    }

    async fn expect_rejection<M>(&self, model: &M, command: C, state: S) -> (Vec<N>, NonEmpty<R>)
    where
        M: DomainModel<State = S, Event = E, Rejection = R> + Sync,
        S: Debug,
        E: Debug,
        R: Debug,
        N: Debug,
    {
        match self.run_with(model, command, state).await {
            EdomatonResult::Rejected {
                notifications,
                reasons,
            } => (notifications, reasons),
            other => panic!("Expected rejection, but obtained: {other:?}"),
        }
    }

    async fn expect_rejection_with<M>(
        &self,
        model: &M,
        command: C,
        state: S,
        expected_errors: impl IntoIterator<Item = R>,
    ) where
        M: DomainModel<State = S, Event = E, Rejection = R> + Sync,
        S: Debug,
        E: Debug,
        R: PartialEq + Debug,
        N: PartialEq + Debug,
    {
        self.expect_rejection_and_notify(model, command, state, expected_errors, [])
            .await
    }

    async fn expect_rejection_and_notify<M>(
        &self,
        model: &M,
        command: C,
        state: S,
        expected_errors: impl IntoIterator<Item = R>,
        expected_notifications: impl IntoIterator<Item = N>,
    ) where
        M: DomainModel<State = S, Event = E, Rejection = R> + Sync,
        S: Debug,
        E: Debug,
        R: PartialEq + Debug,
        N: PartialEq + Debug,
    {
        let (notifications, reasons) = self.expect_rejection(model, command, state).await;
        assert_eq!(
            reasons.into_vec(),
            collect(expected_errors),
            "unexpected rejection reasons"
        );
        assert_eq!(
            notifications,
            collect(expected_notifications),
            "unexpected notifications"
        );
    }

    async fn expect_rejection_notify<M>(
        &self,
        model: &M,
        command: C,
        state: S,
        notifications: impl IntoIterator<Item = N>,
    ) where
        M: DomainModel<State = S, Event = E, Rejection = R> + Sync,
        S: Debug,
        E: Debug,
        R: Debug,
        N: PartialEq + Debug,
    {
        let (actual, _) = self.expect_rejection(model, command, state).await;
        assert_eq!(actual, collect(notifications), "unexpected notifications");
    }

    async fn expect_that<M>(
        &self,
        model: &M,
        command: C,
        state: S,
        expected_notifications: impl IntoIterator<Item = N>,
        that: impl FnOnce(&S),
    ) where
        M: DomainModel<State = S, Event = E, Rejection = R> + Sync,
        S: Debug,
        E: Debug,
        R: Debug,
        N: PartialEq + Debug,
    {
        match self.run_with(model, command, state).await {
            EdomatonResult::Accepted {
                new_state,
                notifications,
                ..
            } => {
                assert_eq!(
                    notifications,
                    collect(expected_notifications),
                    "unexpected notifications"
                );
                that(&new_state);
            }
            other => panic!("Expected success, but obtained: {other:?}"),
        }
    }
}

/// Assertion helpers for CQRS programs (no Scala equivalent; `DomainSuite`
/// only covers `Edomaton`).
#[allow(async_fn_in_trait)]
pub trait StomatonAssertions<C, S, R, N> {
    /// Runs the program with `command` (default [`TestCommand`]) and `state`.
    async fn run_with(&self, command: C, state: S) -> ResponseE<R, N, (S, ())>;

    /// Asserts that the program succeeds with exactly `expected_state` and
    /// `expected_notifications`.
    async fn expect(
        &self,
        command: C,
        state: S,
        expected_state: S,
        expected_notifications: impl IntoIterator<Item = N>,
    ) where
        S: PartialEq + Debug,
        R: Debug,
        N: PartialEq + Debug;

    /// Asserts that the program rejects with exactly `expected_errors` and
    /// `expected_notifications`.
    async fn expect_rejection_with(
        &self,
        command: C,
        state: S,
        expected_errors: impl IntoIterator<Item = R>,
        expected_notifications: impl IntoIterator<Item = N>,
    ) where
        S: Debug,
        R: PartialEq + Debug,
        N: PartialEq + Debug;
}

impl<C, S, R, N> StomatonAssertions<C, S, R, N> for Stomaton<CommandMessage<C>, S, R, N, ()>
where
    C: Send + 'static,
    S: Send + 'static,
    R: Send + 'static,
    N: Send + 'static,
{
    async fn run_with(&self, command: C, state: S) -> ResponseE<R, N, (S, ())> {
        self.run(TestCommand::default().message(command), state)
            .await
    }

    async fn expect(
        &self,
        command: C,
        state: S,
        expected_state: S,
        expected_notifications: impl IntoIterator<Item = N>,
    ) where
        S: PartialEq + Debug,
        R: Debug,
        N: PartialEq + Debug,
    {
        let out = self.run_with(command, state).await;
        match out.result {
            Ok((new_state, ())) => {
                assert_eq!(new_state, expected_state, "unexpected state");
                assert_eq!(
                    out.notifications,
                    collect(expected_notifications),
                    "unexpected notifications"
                );
            }
            Err(reasons) => panic!("Expected success, but obtained rejection: {reasons:?}"),
        }
    }

    async fn expect_rejection_with(
        &self,
        command: C,
        state: S,
        expected_errors: impl IntoIterator<Item = R>,
        expected_notifications: impl IntoIterator<Item = N>,
    ) where
        S: Debug,
        R: PartialEq + Debug,
        N: PartialEq + Debug,
    {
        let out = self.run_with(command, state).await;
        match out.result {
            Err(reasons) => {
                assert_eq!(
                    reasons.into_vec(),
                    collect(expected_errors),
                    "unexpected rejection reasons"
                );
                assert_eq!(
                    out.notifications,
                    collect(expected_notifications),
                    "unexpected notifications"
                );
            }
            Ok((state, ())) => {
                panic!("Expected rejection, but obtained success with state {state:?}")
            }
        }
    }
}
