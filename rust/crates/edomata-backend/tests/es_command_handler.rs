//! Port of `eventsourcing/CommandHandlerSuite.scala`.
//!
//! Adaptation: Rust programs have no error channel, so the Scala tests that
//! raise errors from inside the program ("Must not change raised errors",
//! "Must retry on version conflict") are ported with repositories that fail
//! instead. The intent (errors pass through unchanged; version conflicts are
//! retried up to the limit) is unchanged.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::{DateTime, Utc};
use common::*;
use edomata_backend::eventsourcing::{AggregateState, CommandHandler, CommandState};
use edomata_backend::{BackendError, EventMessage, EventMetadata, RetryConfig, SharedModel};
use edomata_core::{CommandMessage, Decision, DomainModel, NonEmpty, nonempty};
use uuid::Uuid;

type State = String;
type Event = i32;
type Notification = i32;
type Rejection = String;
type Command = i32;
type App = edomata_core::App<Command, State, Event, Rejection, Notification, ()>;
type Repo = FakeRepository<State, Event, Rejection, Notification>;
type Handler = CommandHandler<State, Event, Rejection, Notification>;

struct Sut;

impl DomainModel for Sut {
    type State = State;
    type Event = Event;
    type Rejection = Rejection;

    fn initial(&self) -> State {
        String::new()
    }

    fn transition(&self, i: &Event, s: State) -> Result<State, NonEmpty<Rejection>> {
        if *i > s.len() as i32 {
            Ok(format!("{s}{i}"))
        } else {
            Err(NonEmpty::new("bad event".to_string()))
        }
    }
}

fn model() -> SharedModel<State, Event, Rejection> {
    Arc::new(Sut)
}

fn dsl() -> edomata_core::DomainDsl<Command, State, Event, Rejection, Notification> {
    Sut.dsl()
}

fn cmd() -> CommandMessage<Command> {
    CommandMessage::new("", max_time(), "", 1)
}

fn repo(state: CommandState<State, Event, Rejection>) -> Arc<Repo> {
    Arc::new(Repo::new(state))
}

fn handler(r: Arc<Repo>) -> Handler {
    CommandHandler::new(r, model())
}

#[tokio::test]
async fn ignores_redundant_command() {
    let flag = Arc::new(AtomicBool::new(false));
    let f = Arc::clone(&flag);
    let app: App = dsl().eval(move || {
        let f = Arc::clone(&f);
        async move { f.store(true, Ordering::SeqCst) }
    });
    let r = repo(CommandState::Redundant);
    let s = handler(r.clone()).compile(app);
    assert_eq!(s(cmd()).await, Ok(Ok(())));
    assert!(r.actions().is_empty());
    assert!(!flag.load(Ordering::SeqCst));
}

#[tokio::test]
async fn appends_accepted_results() {
    let app: App = dsl()
        .decide(Decision::accept(nonempty![1, 2, 3]))
        .publish([4, 5, 6]);
    let version = 100;
    let r = repo(valid(String::new(), version));
    let s = handler(r.clone()).compile(app);
    assert_eq!(s(cmd()).await, Ok(Ok(())));
    assert_eq!(
        r.actions(),
        vec![Action::Appended {
            cmd: erased(&cmd()),
            version,
            new_state: "123".to_string(),
            events: nonempty![1, 2, 3],
            notifications: vec![4, 5, 6],
        }]
    );
}

#[tokio::test]
async fn notifies_indecisive_results() {
    let app: App = dsl().decide(Decision::unit()).publish([4, 5, 6]);
    let r = repo(valid(String::new(), 100));
    let s = handler(r.clone()).compile(app);
    assert_eq!(s(cmd()).await, Ok(Ok(())));
    assert_eq!(
        r.actions(),
        vec![Action::Notified {
            cmd: erased(&cmd()),
            notifications: nonempty![4, 5, 6],
        }]
    );
}

#[tokio::test]
async fn notifies_rejection_with_notification() {
    let app: App = dsl().reject("oops!".to_string()).publish([4, 5, 6]);
    let r = repo(valid(String::new(), 100));
    let s = handler(r.clone()).compile(app);
    assert_eq!(s(cmd()).await, Ok(Err(nonempty!["oops!".to_string()])));
    assert_eq!(
        r.actions(),
        vec![Action::Notified {
            cmd: erased(&cmd()),
            notifications: nonempty![4, 5, 6],
        }]
    );
}

#[tokio::test]
async fn rejections_with_no_notifications_have_no_effect() {
    let app: App = dsl().reject("oops!".to_string());
    let r = repo(valid(String::new(), 100));
    let s = handler(r.clone()).compile(app);
    assert_eq!(s(cmd()).await, Ok(Err(nonempty!["oops!".to_string()])));
    assert!(r.actions().is_empty());
}

#[tokio::test]
async fn indecisives_with_no_notifications_have_no_effect() {
    let app: App = dsl().unit();
    let r = repo(valid(String::new(), 100));
    let s = handler(r.clone()).compile(app);
    assert_eq!(s(cmd()).await, Ok(Ok(())));
    assert!(r.actions().is_empty());
}

#[tokio::test]
async fn must_reject_results_that_cause_state_to_become_conflicted() {
    let app: App = dsl().decide(Decision::accept(nonempty![-1, -2]));
    let r = repo(valid(String::new(), 100));
    let s = handler(r.clone()).compile(app);
    assert_eq!(s(cmd()).await, Ok(Err(nonempty!["bad event".to_string()])));
    assert!(r.actions().is_empty());
}

#[tokio::test]
async fn must_reject_working_on_conflicted_state() {
    let app: App = dsl().unit();
    let meta = EventMetadata {
        id: Uuid::new_v4(),
        time: DateTime::<Utc>::MAX_UTC,
        seq_nr: 42,
        version: 16,
        stream: "sut".into(),
    };
    let rejection = "don't know what to do".to_string();
    let r = repo(CommandState::Aggregate(AggregateState::Conflicted {
        last: String::new(),
        on_event: EventMessage {
            metadata: meta,
            payload: -1,
        },
        errors: NonEmpty::new(rejection.clone()),
    }));
    let s = handler(r.clone()).compile(app);
    assert_eq!(s(cmd()).await, Ok(Err(NonEmpty::new(rejection))));
    assert!(r.actions().is_empty());
}

#[tokio::test]
async fn must_not_change_raised_errors() {
    let app: App = dsl().unit();
    let error = BackendError::PersistenceError("Some error!".into());
    let r: Arc<ErrorRepository> = Arc::new(ErrorRepository(clone_error(&error)));
    let s = CommandHandler::<State, Event, Rejection, Notification>::new(r, model()).compile(app);
    assert_eq!(s(cmd()).await, Err(error));
}

#[tokio::test(start_paused = true)]
async fn must_retry_on_version_conflict() {
    let app: App = dsl().decide(Decision::accept(1));
    // Two appends conflict, the third succeeds: three attempts are needed.
    let r = Arc::new(Repo::conflicting(valid(String::new(), 0), 2));
    let s = CommandHandler::with_retry(
        r.clone(),
        model(),
        RetryConfig {
            max_retry: 3,
            initial_delay: Duration::from_secs(60),
        },
    )
    .compile(app);
    assert_eq!(s(cmd()).await, Ok(Ok(())));
    assert_eq!(r.actions().len(), 1);
    assert_eq!(r.loaded().len(), 3);
}

#[tokio::test(start_paused = true)]
async fn must_fail_with_max_retry_on_too_many_version_conflicts() {
    let app: App = dsl().decide(Decision::accept(1));
    // Three appends conflict but only three attempts are allowed.
    let r = Arc::new(Repo::conflicting(valid(String::new(), 0), 3));
    let s = CommandHandler::with_retry(
        r.clone(),
        model(),
        RetryConfig {
            max_retry: 3,
            initial_delay: Duration::from_secs(60),
        },
    )
    .compile(app);
    assert_eq!(s(cmd()).await, Err(BackendError::MaxRetryExceeded));
    assert!(r.actions().is_empty());
    assert_eq!(r.loaded().len(), 3);
}
