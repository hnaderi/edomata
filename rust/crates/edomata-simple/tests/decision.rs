//! Port of `JDecisionSuite.scala`.

use edomata_simple::SimpleDecision;

type D<A> = SimpleDecision<String, String, A>;

#[test]
fn accept_creates_accepted_with_events() {
    let d: D<()> =
        SimpleDecision::<String, String, ()>::accept(["e1".to_string(), "e2".to_string()]);
    assert!(d.is_accepted());
    assert!(!d.is_rejected());
    assert!(!d.is_indecisive());
    match &d {
        SimpleDecision::Accepted { events, result } => {
            assert_eq!(events, &["e1".to_string(), "e2".to_string()]);
            assert_eq!(*result, ());
        }
        other => panic!("Expected Accepted, got {other:?}"),
    }
}

#[test]
fn accept_return_creates_accepted_with_value_and_events() {
    let d: D<i32> = SimpleDecision::accept_return(42, ["e1".to_string()]);
    assert!(d.is_accepted());
    match d {
        SimpleDecision::Accepted { events, result } => {
            assert_eq!(events.len(), 1);
            assert_eq!(result, 42);
        }
        other => panic!("Expected Accepted, got {other:?}"),
    }
}

#[test]
fn reject_creates_rejected_with_reasons() {
    let d: D<()> = SimpleDecision::reject(["r1".to_string(), "r2".to_string(), "r3".to_string()]);
    assert!(d.is_rejected());
    assert!(!d.is_accepted());
    match &d {
        SimpleDecision::Rejected { reasons } => {
            assert_eq!(reasons.len(), 3);
            assert_eq!(reasons[0], "r1");
        }
        other => panic!("Expected Rejected, got {other:?}"),
    }
    assert_eq!(
        d.reasons(),
        &["r1".to_string(), "r2".to_string(), "r3".to_string()]
    );
    assert!(d.events().is_empty());
}

#[test]
fn pure_creates_indecisive() {
    let d: D<String> = SimpleDecision::pure("hello".to_string());
    assert!(d.is_indecisive());
    assert!(!d.is_accepted());
    assert!(!d.is_rejected());
    assert_eq!(
        d,
        SimpleDecision::Indecisive {
            result: "hello".to_string()
        }
    );
}

#[test]
fn unit_is_indecisive_with_unit() {
    assert!(SimpleDecision::<String, String, ()>::unit().is_indecisive());
}

#[test]
fn map_transforms_the_result_value() {
    let d: D<i32> = SimpleDecision::pure(10);
    assert_eq!(
        d.map(|x| format!("v={x}")),
        SimpleDecision::Indecisive {
            result: "v=10".to_string()
        }
    );
}

#[test]
fn map_on_accepted_preserves_events() {
    let d: D<i32> = SimpleDecision::accept_return(5, ["e1".to_string()]);
    match d.map(|x| format!("n={x}")) {
        SimpleDecision::Accepted { events, result } => {
            assert_eq!(events.len(), 1);
            assert_eq!(result, "n=5");
        }
        other => panic!("Expected Accepted, got {other:?}"),
    }
}

#[test]
fn map_on_rejected_is_a_no_op() {
    let d: D<()> = SimpleDecision::reject(["fail".to_string()]);
    assert!(d.map(|_| "never").is_rejected());
}

#[test]
fn and_then_chains_accepted_decisions_merging_events() {
    let d1: D<()> = SimpleDecision::<String, String, ()>::accept(["e1".to_string()]);
    let d2 = d1.and_then(|()| SimpleDecision::<String, String, ()>::accept(["e2".to_string()]));
    assert_eq!(d2.events(), &["e1".to_string(), "e2".to_string()]);
}

#[test]
fn and_then_short_circuits_on_rejected() {
    let d1: D<()> = SimpleDecision::reject(["r1".to_string()]);
    let d2 = d1.and_then(|()| SimpleDecision::<String, String, ()>::pure(()));
    assert!(d2.is_rejected());
}

#[test]
fn and_then_accepted_then_rejected_yields_rejected() {
    let d: D<()> = SimpleDecision::<String, String, ()>::accept(["e1".to_string()]);
    let r = d.flat_map(|()| SimpleDecision::<String, String, ()>::reject(["r1".to_string()]));
    assert!(r.is_rejected());
}

#[test]
fn and_then_indecisive_delegates_to_function_result() {
    let d: D<i32> = SimpleDecision::pure(1);
    let r = d.and_then(|_| SimpleDecision::<String, String, ()>::accept(["e1".to_string()]));
    assert!(r.is_accepted());
    // Accepted then Indecisive keeps the events and takes the new result.
    let d: D<()> = SimpleDecision::<String, String, ()>::accept(["e1".to_string()]);
    let r = d.and_then(|()| SimpleDecision::<String, String, i32>::pure(7));
    assert_eq!(
        r,
        SimpleDecision::Accepted {
            events: vec!["e1".to_string()],
            result: 7
        }
    );
}

#[test]
fn to_result_returns_ok_for_accepted() {
    let d: D<String> = SimpleDecision::accept_return("ok".to_string(), ["e1".to_string()]);
    assert_eq!(d.to_result(), Ok("ok".to_string()));
}

#[test]
fn to_result_returns_err_for_rejected() {
    let d: D<()> = SimpleDecision::reject(["r1".to_string()]);
    assert_eq!(d.to_result(), Err(vec!["r1".to_string()]));
}

#[test]
fn to_result_returns_ok_for_indecisive() {
    let d: D<i32> = SimpleDecision::pure(42);
    assert_eq!(d.to_result(), Ok(42));
}
