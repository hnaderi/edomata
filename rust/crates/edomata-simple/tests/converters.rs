//! Port of `ConvertersSuite.scala`. The Java-list conversions map to
//! `NonEmpty::from_vec` / `into_vec`; the decision conversions to
//! `From<Decision>` and `SimpleDecision::into_decision`.

use edomata_core::{Decision, NonEmpty};
use edomata_simple::{EmptyRejection, SimpleDecision};

#[test]
fn non_empty_to_vec() {
    let nec = NonEmpty::of("x", ["y"]);
    let v = nec.into_vec();
    assert_eq!(v, vec!["x", "y"]);
}

#[test]
fn vec_to_non_empty() {
    assert_eq!(
        NonEmpty::from_vec(vec![1, 2, 3]).map(NonEmpty::into_vec),
        Some(vec![1, 2, 3])
    );
    assert_eq!(NonEmpty::<i32>::from_vec(Vec::new()), None);
}

#[test]
fn accepted_round_trips_through_converters() {
    let original: Decision<String, &str, i32> = Decision::Accepted {
        events: NonEmpty::of("e1", ["e2"]),
        result: 42,
    };
    let simple = SimpleDecision::from(original.clone());
    assert!(simple.is_accepted());
    assert_eq!(simple.into_decision(), Ok(original));
}

#[test]
fn rejected_round_trips_through_converters() {
    let original: Decision<&str, (), ()> = Decision::Rejected(NonEmpty::new("r1"));
    let simple = SimpleDecision::from(original.clone());
    assert!(simple.is_rejected());
    assert_eq!(Decision::try_from(simple), Ok(original));
}

#[test]
fn indecisive_round_trips_through_converters() {
    let original: Decision<(), (), &str> = Decision::InDecisive("ok");
    let simple = SimpleDecision::from(original.clone());
    assert!(simple.is_indecisive());
    assert_eq!(simple.into_decision(), Ok(original));
}

#[test]
fn accepted_without_events_becomes_indecisive_and_empty_rejection_is_an_error() {
    let d: SimpleDecision<&str, &str, i32> = SimpleDecision::Accepted {
        events: Vec::new(),
        result: 1,
    };
    assert_eq!(d.into_decision(), Ok(Decision::InDecisive(1)));
    let d: SimpleDecision<&str, &str, i32> = SimpleDecision::Rejected {
        reasons: Vec::new(),
    };
    assert_eq!(d.into_decision(), Err(EmptyRejection));
    assert_eq!(
        EmptyRejection.to_string(),
        "Rejected must have at least one reason"
    );
}
