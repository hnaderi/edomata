//! Port of `cqrs/CommandHandlerSuite.scala`.
//!
//! Adaptation: Rust programs have no error channel, so the Scala tests that
//! raise errors from inside the program are ported with repositories that
//! fail instead (see `es_command_handler.rs`).

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use common::*;
use edomata_backend::cqrs::{AggregateState, CommandHandler, CommandState};
use edomata_backend::{BackendError, RetryConfig};
use edomata_core::{CommandMessage, CqrsModel, Stomaton, nonempty};

type State = String;
type Event = i32;
type Rejection = String;
type Command = i32;
type App = Stomaton<CommandMessage<Command>, State, Rejection, Event, ()>;
type Repo = CqrsFakeRepository<State, Event>;

struct Sut;

impl CqrsModel for Sut {
    type State = State;
    type Rejection = Rejection;

    fn initial(&self) -> State {
        String::new()
    }
}

fn dsl() -> edomata_core::CqrsDomainDsl<Command, State, Rejection, Event> {
    Sut.dsl()
}

fn cmd() -> CommandMessage<Command> {
    CommandMessage::new("", max_time(), "", 1)
}

fn repo(state: CommandState<State>) -> Arc<Repo> {
    Arc::new(Repo::new(state))
}

fn agg(s: &str, v: i64) -> CommandState<State> {
    CommandState::Aggregate(AggregateState::new(s.to_string(), v))
}

fn handler(r: Arc<Repo>) -> CommandHandler<State, Event> {
    CommandHandler::new(r)
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
    assert!(r.saved().is_empty());
    assert!(!flag.load(Ordering::SeqCst));
}

#[tokio::test]
async fn saves_accepted_results() {
    let app: App = dsl().publish([1, 2, 3]).then(dsl().set("123".into()));
    let r = repo(agg("", 100));
    let s = handler(r.clone()).compile(app);
    assert_eq!(s(cmd()).await, Ok(Ok(())));
    assert_eq!(
        r.saved(),
        vec![Interaction::Saved {
            cmd: erased(&cmd()),
            version: 100,
            new_state: "123".into(),
            events: vec![1, 2, 3],
        }]
    );
}

#[tokio::test]
async fn saves_results_even_if_indecisive() {
    let app: App = Stomaton::set("123".into());
    let r = repo(agg("", 100));
    let s = handler(r.clone()).compile(app);
    assert_eq!(s(cmd()).await, Ok(Ok(())));
    assert_eq!(
        r.saved(),
        vec![Interaction::Saved {
            cmd: erased(&cmd()),
            version: 100,
            new_state: "123".into(),
            events: vec![],
        }]
    );
}

#[tokio::test]
async fn does_not_save_results_when_rejected() {
    let app: App = Stomaton::reject(nonempty!["a".to_string(), "b".to_string(), "c".to_string()]);
    let r = repo(agg("", 100));
    let s = handler(r.clone()).compile(app);
    assert_eq!(
        s(cmd()).await,
        Ok(Err(nonempty![
            "a".to_string(),
            "b".to_string(),
            "c".to_string()
        ]))
    );
    assert!(r.saved().is_empty());
}

#[tokio::test]
async fn notifies_rejection_events() {
    let app: App = Stomaton::reject(nonempty!["a".to_string(), "b".to_string(), "c".to_string()])
        .publish([1, 2, 3]);
    let r = repo(agg("", 100));
    let s = handler(r.clone()).compile(app);
    assert_eq!(
        s(cmd()).await,
        Ok(Err(nonempty![
            "a".to_string(),
            "b".to_string(),
            "c".to_string()
        ]))
    );
    assert_eq!(
        r.saved(),
        vec![Interaction::Notified {
            cmd: erased(&cmd()),
            events: nonempty![1, 2, 3],
        }]
    );
}

#[tokio::test]
async fn must_not_change_raised_errors() {
    let app: App = dsl().unit();
    let s = CommandHandler::<State, Event>::new(Arc::new(CqrsFailingRepository)).compile(app);
    assert_eq!(s(cmd()).await, Err(planned_failure()));
}

#[tokio::test(start_paused = true)]
async fn must_retry_on_version_conflict() {
    let app: App = dsl().unit();
    let r = Arc::new(Repo::conflicting(agg("", 0), 2));
    let s = CommandHandler::with_retry(
        r.clone(),
        RetryConfig {
            max_retry: 3,
            initial_delay: Duration::from_secs(60),
        },
    )
    .compile(app);
    assert_eq!(s(cmd()).await, Ok(Ok(())));
    assert_eq!(
        r.saved(),
        vec![Interaction::Saved {
            cmd: erased(&cmd()),
            version: 0,
            new_state: String::new(),
            events: vec![],
        }]
    );
}

#[tokio::test(start_paused = true)]
async fn must_fail_with_max_retry_on_too_many_version_conflicts() {
    let app: App = dsl().unit();
    let r = Arc::new(Repo::conflicting(agg("", 0), 3));
    let s = CommandHandler::with_retry(
        r.clone(),
        RetryConfig {
            max_retry: 3,
            initial_delay: Duration::from_secs(60),
        },
    )
    .compile(app);
    assert_eq!(s(cmd()).await, Err(BackendError::MaxRetryExceeded));
    assert!(r.saved().is_empty());
}
