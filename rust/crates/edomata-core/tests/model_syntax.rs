//! Port of `ModelSyntaxSuite.scala`.

use edomata_core::{Decision, DomainModel, NonEmpty, nonempty};

struct Model;

impl DomainModel for Model {
    type State = i32;
    type Event = i32;
    type Rejection = String;

    fn initial(&self) -> i32 {
        0
    }

    fn transition(&self, event: &i32, state: i32) -> Result<i32, NonEmpty<String>> {
        if *event > 0 {
            Ok(event + state)
        } else {
            Err(NonEmpty::new("Invalid number!".to_string()))
        }
    }
}

#[test]
fn accept_events() {
    assert_eq!(Model.accept(1, 2), Model.perform(1, Decision::accept(2)));
    assert_eq!(Model.accept(1, 2), Decision::accept_return(3, 2));
    assert_eq!(
        Model.accept(1, nonempty![2, 3]),
        Model.perform(1, Decision::accept(nonempty![2, 3]))
    );
    assert_eq!(
        Model.accept(1, nonempty![2, 3]),
        Decision::accept_return(6, nonempty![2, 3])
    );
}

#[test]
fn transition_failure_becomes_rejection() {
    assert_eq!(
        Model.accept(1, nonempty![2, -1]),
        Decision::Rejected(nonempty!["Invalid number!".to_string()])
    );
}

#[test]
fn handle_returns_state_and_output() {
    assert_eq!(
        Model.handle(1, Decision::accept_return("out", 2)),
        Decision::accept_return((3, "out"), 2)
    );
    assert_eq!(
        Model.handle(1, Decision::pure("out")),
        Decision::pure((1, "out"))
    );
    let rejected: Decision<String, i32, &str> = Decision::reject("no".to_string());
    assert_eq!(
        Model.handle(1, rejected),
        Decision::Rejected(nonempty!["no".to_string()])
    );
}

#[test]
fn decide_and_decide_return() {
    assert_eq!(
        Model.decide(1, |s| Decision::accept_return("x", *s + 1)),
        Decision::accept_return(3, 2)
    );
    assert_eq!(
        Model.decide_return(1, |s| Decision::accept_return("x", *s + 1)),
        Decision::accept_return((3, "x"), 2)
    );
    assert_eq!(Model.initial(), 0);
}
