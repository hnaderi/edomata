//! Port of `decision/DecisionSyntaxSuite.scala`.

use edomata_core::syntax::*;
use edomata_core::{Decision, NonEmpty, nonempty};

type D<A> = Decision<i32, i32, A>;

#[test]
fn decision_pure() {
    assert_eq!(D::pure(1), Decision::InDecisive(1));
}

#[test]
fn decision_accept() {
    assert_eq!(
        D::accept(nonempty![1, 2, 3]),
        Decision::Accepted {
            events: nonempty![1, 2, 3],
            result: ()
        }
    );
}

#[test]
fn decision_accept_return() {
    assert_eq!(
        D::accept_return(0, nonempty![1, 2, 3]),
        Decision::Accepted {
            events: nonempty![1, 2, 3],
            result: 0
        }
    );
}

#[test]
fn decision_reject() {
    assert_eq!(
        D::<()>::reject(nonempty![1, 2, 3]),
        Decision::Rejected(nonempty![1, 2, 3])
    );
}

#[test]
fn value_accept() {
    assert_eq!(1.accept::<i32>(), D::accept(1));
}

#[test]
fn value_reject() {
    assert_eq!(1.reject::<i32, ()>(), D::<()>::reject(1));
}

#[test]
fn value_into_decision() {
    assert_eq!(1.into_decision::<i32, i32>(), D::pure(1));
}

#[test]
fn option_to_decision() {
    assert_eq!(Some(1).to_decision::<i32, i32>(0), D::pure(1));
    assert_eq!(None::<i32>.to_decision::<i32, i32>(0), D::reject(0));
    assert_eq!(
        None::<i32>.to_decision::<i32, i32>(nonempty![0, 1]),
        D::reject(nonempty![0, 1])
    );
}

#[test]
fn option_to_accepted() {
    assert_eq!(Some(1).to_accepted::<i32>(), D::accept(1));
    assert_eq!(None::<i32>.to_accepted::<i32>(), D::unit());
}

#[test]
fn option_to_accepted_or() {
    assert_eq!(Some(1).to_accepted_or::<i32>(nonempty![2, 3]), D::accept(1));
    assert_eq!(
        None::<i32>.to_accepted_or::<i32>(nonempty![2, 3]),
        D::reject(nonempty![2, 3])
    );
}

#[test]
fn option_to_rejected() {
    assert_eq!(Some(1).to_rejected::<i32>(), D::reject(1));
    assert_eq!(None::<i32>.to_rejected::<i32>(), D::unit());
}

#[test]
fn result_to_decision() {
    assert_eq!(Ok::<i32, i32>(1).to_decision::<i32>(), D::pure(1));
    assert_eq!(Err::<i32, i32>(1).to_decision::<i32>(), D::reject(1));
}

#[test]
fn result_to_accepted() {
    assert_eq!(Ok::<i32, i32>(1).to_accepted(), D::accept(1));
    assert_eq!(Err::<i32, i32>(1).to_accepted(), D::reject(1));
}

#[test]
fn result_nec_to_decision() {
    assert_eq!(
        Ok::<i32, NonEmpty<i32>>(1).to_decision_nec::<i32>(),
        D::pure(1)
    );
    assert_eq!(
        Err::<i32, _>(nonempty![1, 2, 3]).to_decision_nec::<i32>(),
        D::reject(nonempty![1, 2, 3])
    );
}

#[test]
fn result_nec_to_accepted() {
    assert_eq!(Ok::<i32, NonEmpty<i32>>(1).to_accepted_nec(), D::accept(1));
    assert_eq!(
        Err::<i32, _>(nonempty![1, 2, 3]).to_accepted_nec(),
        D::reject(nonempty![1, 2, 3])
    );
}

#[test]
fn decision_validate_result() {
    let validate = |i: i32| if i > 0 { Ok(i) } else { Err(i) };
    assert_eq!(D::pure(0).validate_one(validate), D::reject(0));
    assert_eq!(D::pure(1).validate_one(validate), D::pure(1));
}

#[test]
fn decision_validate_result_nec() {
    let validate = |i: i32| if i > 0 { Ok(i) } else { Err(nonempty![i, i]) };
    assert_eq!(
        D::pure(0).validate_with(validate),
        D::reject(nonempty![0, 0])
    );
    assert_eq!(D::pure(1).validate_with(validate), D::pure(1));
}

#[test]
fn decision_accept_when_bool() {
    assert_eq!(D::accept_when(false, nonempty![1, 2, 3]), D::unit());
    assert_eq!(
        D::accept_when(true, nonempty![1, 2, 3]),
        D::accept(nonempty![1, 2, 3])
    );
}

#[test]
fn decision_accept_when_option() {
    assert_eq!(D::accept_some(Some(1)), D::accept(1));
    assert_eq!(D::accept_some(None), D::unit());
    assert_eq!(D::accept_some_or(Some(1), nonempty![2, 3]), D::accept(1));
    assert_eq!(
        D::accept_some_or(None, nonempty![2, 3]),
        D::reject(nonempty![2, 3])
    );
}

#[test]
fn decision_accept_when_result() {
    assert_eq!(D::accept_ok(Ok(1)), D::accept(1));
    assert_eq!(D::accept_ok(Err(1)), D::reject(1));
}

#[test]
fn decision_accept_when_result_nec() {
    assert_eq!(D::accept_ok_nec(Ok(1)), D::accept(1));
    assert_eq!(
        D::accept_ok_nec(Err(nonempty![2, 3])),
        D::reject(nonempty![2, 3])
    );
}

#[test]
fn decision_reject_when() {
    assert_eq!(D::reject_when(false, nonempty![1, 2]), D::unit());
    assert_eq!(
        D::reject_when(true, nonempty![1, 2]),
        D::reject(nonempty![1, 2])
    );
    assert_eq!(D::reject_some(Some(1)), D::reject(1));
    assert_eq!(D::reject_some(None), D::unit());
}

#[test]
fn decision_from_option() {
    assert_eq!(D::from_option(Some(1), 2), D::pure(1));
    assert_eq!(
        D::<i32>::from_option(None, nonempty![2, 3]),
        D::reject(nonempty![2, 3])
    );
}

#[test]
fn decision_from_result() {
    assert_eq!(D::from_result(Ok(1)), D::pure(1));
    assert_eq!(D::<i32>::from_result(Err(1)), D::reject(1));
}

#[test]
fn decision_from_result_nec() {
    assert_eq!(D::from_result_nec(Ok(1)), D::pure(1));
    assert_eq!(
        D::<i32>::from_result_nec(Err(nonempty![2, 3])),
        D::reject(nonempty![2, 3])
    );
}

#[test]
fn decision_validate() {
    assert_eq!(D::validate(Ok(1)), D::pure(1));
    assert_eq!(
        D::<i32>::validate(Err(nonempty![2, 3])),
        D::reject(nonempty![2, 3])
    );
}

#[test]
fn decision_accessors() {
    let d = D::accept_return(1, nonempty![7]);
    assert!(d.is_accepted());
    assert!(!d.is_rejected());
    assert!(!d.is_indecisive());
    assert_eq!(d.result(), Some(&1));
    assert_eq!(d.events(), Some(&nonempty![7]));
    assert_eq!(d.rejections(), None);
    assert_eq!(d.clone().to_option(), Some(1));
    assert_eq!(d.clone().void(), D::accept(7));
    let r = D::<i32>::reject(nonempty![1]);
    assert_eq!(r.rejections(), Some(&nonempty![1]));
    assert_eq!(r.clone().to_option(), None);
    assert_eq!(
        r.clone().map_rejections(|x| x + 1),
        Decision::Rejected(nonempty![2])
    );
    assert_eq!(
        d.map_events(|e| e.to_string()),
        Decision::accept_return(1, "7".to_string())
    );
}
