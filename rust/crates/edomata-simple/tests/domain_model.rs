//! Port of `JDomainModelSuite.scala`.

use edomata_core::{Decision, DomainModel, NonEmpty};
use edomata_simple::{ClosureModel, SimpleDomainModel};

/// The counter domain of the Scala suite, implemented directly.
struct Counter;

impl SimpleDomainModel for Counter {
    type State = i32;
    type Event = String;
    type Rejection = String;

    fn initial(&self) -> i32 {
        0
    }

    fn transition(&self, event: &String, state: i32) -> Result<i32, Vec<String>> {
        if let Some(n) = event.strip_prefix('+') {
            Ok(state + n.parse::<i32>().unwrap())
        } else if let Some(n) = event.strip_prefix('-') {
            let amount = n.parse::<i32>().unwrap();
            if state >= amount {
                Ok(state - amount)
            } else {
                Err(vec!["insufficient balance".to_string()])
            }
        } else {
            Err(vec!["unknown event".to_string()])
        }
    }
}

#[test]
fn initial_state_is_correct() {
    assert_eq!(Counter.initial(), 0);
}

#[test]
fn transition_returns_ok_for_valid_event() {
    assert_eq!(Counter.transition(&"+5".to_string(), 10), Ok(15));
}

#[test]
fn transition_returns_err_for_invalid_event() {
    assert!(Counter.transition(&"-20".to_string(), 10).is_err());
}

#[test]
fn into_model_produces_a_valid_domain_model() {
    let model = Counter.into_model();
    assert_eq!(DomainModel::initial(&model), 0);
    assert_eq!(
        DomainModel::transition(&model, &"+3".to_string(), 7),
        Ok(10)
    );
}

#[test]
fn into_model_transition_rejects_correctly() {
    let model = Counter.into_model();
    assert_eq!(
        DomainModel::transition(&model, &"-100".to_string(), 5),
        Err(NonEmpty::new("insufficient balance".to_string()))
    );
}

#[test]
fn into_model_performs_decisions_correctly() {
    let model = Counter.into_model();
    let decision: Decision<String, String, ()> = Decision::accept("+10".to_string());
    match model.perform(0, decision) {
        Decision::Accepted { result, .. } => assert_eq!(result, 10),
        other => panic!("Expected Accepted, got {other:?}"),
    }
}

#[test]
fn create_factory_works() {
    let model: ClosureModel<i32, String, String> =
        ClosureModel::new(0, |event: &String, state| Ok(state + event.len() as i32));
    assert_eq!(model.initial(), 0);
    assert_eq!(model.transition(&"hello".to_string(), 5), Ok(10));
    let model2: ClosureModel<i32, String, String> = ClosureModel::new(1, |_, s| Ok(s));
    assert_eq!(model2.clone().initial(), 1);
    assert!(format!("{model2:?}").contains("initial: 1"));
}

#[test]
#[should_panic(expected = "Rejection must have at least one reason")]
fn empty_rejection_list_is_a_programming_error() {
    let model: ClosureModel<i32, String, String> = ClosureModel::new(0, |_, _| Err(Vec::new()));
    let _ = DomainModel::transition(&model.into_model(), &"x".to_string(), 0);
}
