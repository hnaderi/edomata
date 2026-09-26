//! Tests for the testkit helpers (the Scala `munit` module has no suite of
//! its own; these pin the behaviour of every `DomainSuite` helper).

use edomata_core::*;
use edomata_testkit::{EdomatonAssertions, StomatonAssertions, TestCommand};
use futures::executor::block_on;

struct Counter;

impl DomainModel for Counter {
    type State = i32;
    type Event = i32;
    type Rejection = String;

    fn initial(&self) -> i32 {
        0
    }

    fn transition(&self, e: &i32, s: i32) -> Result<i32, NonEmpty<String>> {
        Ok(s + e)
    }
}

type Dsl = DomainDsl<i32, i32, i32, String, String>;

fn dsl() -> Dsl {
    Counter.dsl()
}

fn app() -> App<i32, i32, i32, String, String, ()> {
    let d = dsl();
    d.router(move |by| match by {
        0 => d
            .reject("zero".to_string())
            .publish(["rejected".to_string()]),
        -1 => d.reject("negative".to_string()),
        _ => d
            .accept(by)
            .publish(["changed".to_string(), "audited".to_string()]),
    })
}

#[test]
fn run_with_uses_default_command() {
    let d = dsl();
    let ids = d
        .message_id()
        .and_then(move |id| d.aggregate_id().map(move |a| format!("{id}/{a}")));
    let ctx_app: App<i32, i32, i32, String, String, ()> = ids.and_then(move |s| {
        let d = dsl();
        d.publish([s])
    });
    block_on(async {
        match ctx_app.run_with(&Counter, 1, 0).await {
            EdomatonResult::Indecisive { notifications } => {
                assert_eq!(notifications, vec!["1/sut".to_string()])
            }
            other => panic!("{other:?}"),
        }
        match ctx_app
            .run_with_command(&Counter, &TestCommand::new("cmd-9", "acc-1"), 1, 0)
            .await
        {
            EdomatonResult::Indecisive { notifications } => {
                assert_eq!(notifications, vec!["cmd-9/acc-1".to_string()])
            }
            other => panic!("{other:?}"),
        }
    });
}

#[test]
fn expect_checks_state_and_notifications_in_order() {
    block_on(app().expect(
        &Counter,
        5,
        10,
        15,
        ["changed".to_string(), "audited".to_string()],
    ));
}

#[test]
#[should_panic(expected = "unexpected notifications")]
fn expect_fails_on_wrong_notification_order() {
    block_on(app().expect(
        &Counter,
        5,
        10,
        15,
        ["audited".to_string(), "changed".to_string()],
    ));
}

#[test]
#[should_panic(expected = "unexpected state")]
fn expect_fails_on_wrong_state() {
    block_on(app().expect(
        &Counter,
        5,
        10,
        16,
        ["changed".to_string(), "audited".to_string()],
    ));
}

#[test]
#[should_panic(expected = "Expected success")]
fn expect_fails_on_rejection() {
    block_on(app().expect(&Counter, 0, 10, 10, []));
}

#[test]
fn expect_all_ignores_notification_order() {
    block_on(app().expect_all(
        &Counter,
        5,
        10,
        15,
        ["audited".to_string(), "changed".to_string()],
    ));
}

#[test]
fn expect_rejection_returns_notifications_and_reasons() {
    let (ns, reasons) = block_on(app().expect_rejection(&Counter, 0, 10));
    assert_eq!(ns, vec!["rejected".to_string()]);
    assert_eq!(reasons, nonempty!["zero".to_string()]);
}

#[test]
#[should_panic(expected = "Expected rejection")]
fn expect_rejection_fails_on_success() {
    block_on(app().expect_rejection(&Counter, 5, 10));
}

#[test]
fn expect_rejection_with_checks_reasons_and_no_notifications() {
    block_on(app().expect_rejection_with(&Counter, -1, 10, ["negative".to_string()]));
}

#[test]
#[should_panic(expected = "unexpected notifications")]
fn expect_rejection_with_fails_when_notifications_were_published() {
    block_on(app().expect_rejection_with(&Counter, 0, 10, ["zero".to_string()]));
}

#[test]
fn expect_rejection_and_notify_checks_both() {
    block_on(app().expect_rejection_and_notify(
        &Counter,
        0,
        10,
        ["zero".to_string()],
        ["rejected".to_string()],
    ));
}

#[test]
fn expect_rejection_notify_checks_notifications_only() {
    block_on(app().expect_rejection_notify(&Counter, 0, 10, ["rejected".to_string()]));
}

#[test]
fn expect_that_runs_the_predicate_on_the_new_state() {
    block_on(app().expect_that(
        &Counter,
        5,
        10,
        ["changed".to_string(), "audited".to_string()],
        |s| assert!(*s > 10),
    ));
}

struct Tally;

impl CqrsModel for Tally {
    type State = i32;
    type Rejection = String;

    fn initial(&self) -> i32 {
        0
    }
}

#[test]
fn stomaton_assertions() {
    let d = Tally.dsl::<i32, String>();
    let app = d.router(move |by| {
        if by == 0 {
            d.reject("zero".to_string()).publish(["nope".to_string()])
        } else {
            d.modify(move |s| s + by)
                .void()
                .publish(["changed".to_string()])
        }
    });
    block_on(async {
        app.expect(3, 4, 7, ["changed".to_string()]).await;
        app.expect_rejection_with(0, 4, ["zero".to_string()], ["nope".to_string()])
            .await;
        let out = app.run_with(1, 1).await;
        assert_eq!(out.result, Ok((2, ())));
    });
}

#[test]
fn test_command_defaults_match_scala() {
    let t = TestCommand::default();
    assert_eq!(t.msg_id, "1");
    assert_eq!(t.address, "sut");
    assert_eq!(t.time, chrono::DateTime::<chrono::Utc>::MIN_UTC);
    let m = t.message(42);
    assert_eq!(
        (m.id.as_str(), m.address.as_str(), m.payload),
        ("1", "sut", 42)
    );
}
