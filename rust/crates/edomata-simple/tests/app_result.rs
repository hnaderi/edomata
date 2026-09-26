//! Port of `JAppResultSuite.scala`.

use edomata_simple::{AppResult, SimpleDecision};

type Res = AppResult<String, String, String>;

#[test]
fn decide_creates_result_with_decision_and_no_notifications() {
    let r: Res = AppResult::decide(SimpleDecision::<String, String, ()>::accept([
        "e1".to_string()
    ]));
    assert!(r.decision.is_accepted());
    assert!(r.notifications.is_empty());
}

#[test]
fn accept_creates_accepted_result() {
    let r: Res = AppResult::accept(["e1".to_string(), "e2".to_string()]);
    assert!(r.decision.is_accepted());
    assert_eq!(r.decision.events().len(), 2);
}

#[test]
fn reject_creates_rejected_result() {
    let r: Res = AppResult::reject(["r1".to_string()]);
    assert!(r.decision.is_rejected());
}

#[test]
fn publish_creates_indecisive_result_with_notifications() {
    let r: Res = AppResult::publish(["n1".to_string(), "n2".to_string()]);
    assert!(r.decision.is_indecisive());
    assert_eq!(r.notifications.len(), 2);
    assert_eq!(r.notifications[0], "n1");
}

#[test]
fn decide_and_publish_creates_result_with_both() {
    let r: Res = AppResult::decide_and_publish(
        SimpleDecision::<String, String, ()>::accept(["e1".to_string()]),
        ["n1".to_string()],
    );
    assert!(r.decision.is_accepted());
    assert_eq!(r.notifications.len(), 1);
    let r = r.and_publish(["n2".to_string()]);
    assert_eq!(r.notifications, vec!["n1".to_string(), "n2".to_string()]);
}
