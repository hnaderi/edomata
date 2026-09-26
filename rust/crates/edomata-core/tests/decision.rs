//! Port of `decision/DecisionSuite.scala`: MonadError, Traverse and Eq laws
//! plus the accumulation properties.

mod common;

use std::ops::ControlFlow;

use common::*;
use edomata_core::{Decision, NonEmpty, nonempty};
use proptest::prelude::*;

proptest! {
    // ------------------------------------------------------------------
    // Functor laws
    // ------------------------------------------------------------------

    #[test]
    fn functor_identity(a in any_sut()) {
        prop_assert_eq!(a.clone().map(|x| x), a);
    }

    #[test]
    fn functor_composition(a in any_sut(), f in int_fn(), g in int_fn()) {
        prop_assert_eq!(
            a.clone().map(|x| f.call(x)).map(|x| g.call(x)),
            a.map(|x| g.call(f.call(x)))
        );
    }

    // ------------------------------------------------------------------
    // Monad laws
    // ------------------------------------------------------------------

    #[test]
    fn monad_left_identity(a in any::<i64>(), f in dec_fn()) {
        prop_assert_eq!(Decision::pure(a).and_then(|x| f.call(x)), f.call(a));
    }

    #[test]
    fn monad_right_identity(a in any_sut()) {
        prop_assert_eq!(a.clone().and_then(Decision::pure), a);
    }

    #[test]
    fn monad_associativity(a in any_sut(), f in dec_fn(), g in dec_fn()) {
        prop_assert_eq!(
            a.clone().and_then(|x| f.call(x)).and_then(|x| g.call(x)),
            a.and_then(|x| f.call(x).and_then(|y| g.call(y)))
        );
    }

    #[test]
    fn map_consistent_with_and_then(a in any_sut(), f in int_fn()) {
        prop_assert_eq!(
            a.clone().map(|x| f.call(x)),
            a.and_then(|x| Decision::pure(f.call(x)))
        );
    }

    #[test]
    fn tail_rec_consistent_with_and_then(a in any::<i64>(), f in dec_fn()) {
        // tailRecM(a)(f.map(Right(_))) == f(a)
        let via_rec: Sut = Decision::tail_rec(a, |x| f.call(x).map(ControlFlow::Break));
        prop_assert_eq!(via_rec, f.call(a));
    }

    #[test]
    fn tail_rec_accumulates_events(steps in 1usize..20, ev in any::<i32>()) {
        let d: Decision<Rejection, Event, usize> = Decision::tail_rec(0usize, |i| {
            if i < steps {
                Decision::accept_return(ControlFlow::Continue(i + 1), ev)
            } else {
                Decision::pure(ControlFlow::Break(i))
            }
        });
        let expected = NonEmpty::from_vec(vec![ev; steps]).unwrap();
        prop_assert_eq!(d, Decision::Accepted { events: expected, result: steps });
    }

    // ------------------------------------------------------------------
    // MonadError laws
    // ------------------------------------------------------------------

    #[test]
    fn raise_error_handled(e in nec_of(rejection()), f in dec_fn()) {
        let raised: Sut = Decision::Rejected(e.clone());
        prop_assert_eq!(
            raised.handle_error_with(|errs| f.call(errs.len() as i64)),
            f.call(e.len() as i64)
        );
    }

    #[test]
    fn pure_is_not_handled(a in any::<i64>(), f in dec_fn()) {
        let d: Sut = Decision::pure(a);
        prop_assert_eq!(d.clone().handle_error_with(|errs| f.call(errs.len() as i64)), d);
    }

    #[test]
    fn raise_error_short_circuits(e in nec_of(rejection()), f in dec_fn()) {
        let raised: Sut = Decision::Rejected(e.clone());
        prop_assert_eq!(raised.and_then(|x| f.call(x)), Decision::Rejected(e));
    }

    #[test]
    fn to_result_round_trip(a in any_sut()) {
        // attempt/rethrow consistency, ignoring events
        let r = a.clone().to_result();
        match a {
            Decision::Rejected(e) => prop_assert_eq!(r, Err(e)),
            Decision::InDecisive(v) | Decision::Accepted { result: v, .. } => prop_assert_eq!(r, Ok(v)),
        }
    }

    // ------------------------------------------------------------------
    // Traverse laws (via FromIterator and transpose)
    // ------------------------------------------------------------------

    #[test]
    fn traverse_identity(a in any_sut()) {
        prop_assert_eq!(a.clone().map(Some).transpose(), Some(a));
    }

    #[test]
    fn traverse_result_identity(a in any_sut()) {
        prop_assert_eq!(a.clone().map(Ok::<i64, ()>).transpose(), Ok(a));
    }

    #[test]
    fn sequence_of_singleton(a in any_sut()) {
        let seq: Dec<Vec<i64>> = vec![a.clone()].into_iter().collect();
        prop_assert_eq!(seq, a.map(|x| vec![x]));
    }

    #[test]
    fn sequence_is_sequential_composition(a in any_sut(), b in any_sut()) {
        let seq: Dec<Vec<i64>> = vec![a.clone(), b.clone()].into_iter().collect();
        prop_assert_eq!(seq, a.and_then(|x| b.map(|y| vec![x, y])));
    }

    #[test]
    fn fold_left_ignores_rejections(a in any_sut()) {
        let folded = a.fold_left(1i64, |acc, x| acc.wrapping_add(*x));
        match a.as_result() {
            Ok(x) => prop_assert_eq!(folded, 1i64.wrapping_add(*x)),
            Err(_) => prop_assert_eq!(folded, 1),
        }
    }

    // ------------------------------------------------------------------
    // Eq laws
    // ------------------------------------------------------------------

    #[test]
    fn eq_reflexive_and_symmetric(a in any_sut(), b in any_sut()) {
        prop_assert_eq!(a.clone(), a.clone());
        prop_assert_eq!(a == b, b == a);
    }

    // ------------------------------------------------------------------
    // Accumulation properties
    // ------------------------------------------------------------------

    #[test]
    fn accepted_accumulates(a in accepted(), b in accepted()) {
        let c = a.clone().and_then(|_| b.clone());
        let (Decision::Accepted { events: ea, .. }, Decision::Accepted { events: eb, result: rb }) = (a, b) else {
            unreachable!()
        };
        prop_assert_eq!(c, Decision::Accepted { events: ea.concat(eb), result: rb });
    }

    #[test]
    fn rejected_terminates(a in not_rejected(), b in rejected()) {
        prop_assert_eq!(a.and_then(|_| b.clone()), b);
    }

    #[test]
    fn rejected_does_not_change(a in rejected(), b in not_rejected()) {
        prop_assert_eq!(a.clone().and_then(|_| b), a);
    }

    // ------------------------------------------------------------------
    // Validation / assertion consistency
    // ------------------------------------------------------------------

    #[test]
    fn validation_using_result_nec(a in any_sut(), f in dec_fn()) {
        let v = |x: i64| f.call(x).to_result();
        prop_assert_eq!(a.clone().validate_with(v), a.and_then(|x| Decision::validate(v(x))));
    }

    #[test]
    fn validation_using_result(a in any_sut(), f in dec_fn()) {
        let v = |x: i64| f.call(x).to_result().map_err(|e| e.head().clone());
        prop_assert_eq!(a.clone().validate_one(v), a.and_then(|x| Decision::from_result(v(x))));
    }

    #[test]
    fn assertion_using_result_nec(a in any_sut(), f in dec_fn()) {
        let v = |x: &i64| f.call(*x).to_result();
        prop_assert_eq!(a.clone().assert_with(v), a.flat_tap(|x| Decision::validate(v(x))));
    }

    #[test]
    fn assertion_using_result(a in any_sut(), f in dec_fn()) {
        let v = |x: &i64| f.call(*x).to_result().map_err(|e| e.head().clone());
        prop_assert_eq!(a.clone().assert_one(v), a.flat_tap(|x| Decision::from_result(v(x))));
    }
}

#[test]
fn tail_rec_is_stack_safe() {
    let d: Decision<String, i32, u64> = Decision::tail_rec(0u64, |i| {
        if i < 200_000 {
            Decision::pure(ControlFlow::Continue(i + 1))
        } else {
            Decision::pure(ControlFlow::Break(i))
        }
    });
    assert_eq!(d, Decision::pure(200_000));
}

#[test]
fn sequence_short_circuits_on_first_rejection() {
    let ds: Vec<Decision<&str, i32, i32>> = vec![
        Decision::accept_return(1, 1),
        Decision::reject("boom"),
        Decision::accept_return(2, 2),
    ];
    let all: Decision<&str, i32, Vec<i32>> = ds.into_iter().collect();
    assert_eq!(all, Decision::Rejected(nonempty!["boom"]));
}
