//! Port of `decisiont/DecisionTSuite.scala`.

mod common;

use std::ops::ControlFlow;

use common::*;
use edomata_core::{Decision, DecisionT};
use proptest::prelude::*;

type Dtt<T> = DecisionT<Rejection, Event, T>;

fn lift(d: Sut) -> Dtt<i64> {
    DecisionT::lift(d)
}

fn run<T>(d: Dtt<T>) -> Dec<T> {
    block_on(d)
}

proptest! {
    #[test]
    fn accepted_accumulates(a in accepted(), b in accepted()) {
        let b2 = b.clone();
        let c = run(lift(a.clone()).and_then(move |_| lift(b2)));
        let (Decision::Accepted { events: ea, .. }, Decision::Accepted { events: eb, result: rb }) = (a, b) else {
            unreachable!()
        };
        prop_assert_eq!(c, Decision::Accepted { events: ea.concat(eb), result: rb });
    }

    #[test]
    fn rejected_terminates(a in not_rejected(), b in rejected()) {
        let b2 = b.clone();
        let c = run(lift(a).and_then(move |_| lift(b2)));
        prop_assert_eq!(c, b);
    }

    #[test]
    fn rejected_does_not_change(a in rejected(), b in not_rejected()) {
        let c = run(lift(a.clone()).and_then(move |_| lift(b)));
        prop_assert_eq!(c, a);
    }

    // Monad laws, checked by running the future.

    #[test]
    fn monad_left_identity(a in any::<i64>(), f in dec_fn()) {
        let f2 = f.clone();
        prop_assert_eq!(run(Dtt::pure(a).and_then(move |x| lift(f2.call(x)))), f.call(a));
    }

    #[test]
    fn monad_right_identity(a in any_sut()) {
        prop_assert_eq!(run(lift(a.clone()).and_then(DecisionT::pure)), a);
    }

    #[test]
    fn monad_associativity(a in any_sut(), f in dec_fn(), g in dec_fn()) {
        let (f1, g1, f2, g2) = (f.clone(), g.clone(), f, g);
        let lhs = run(lift(a.clone()).and_then(move |x| lift(f1.call(x))).and_then(move |x| lift(g1.call(x))));
        let rhs = run(lift(a).and_then(move |x| lift(f2.call(x)).and_then(move |y| lift(g2.call(y)))));
        prop_assert_eq!(lhs, rhs);
    }

    #[test]
    fn map_consistent_with_decision(a in any_sut(), f in int_fn()) {
        prop_assert_eq!(run(lift(a.clone()).map(move |x| f.call(x))), a.map(|x| f.call(x)));
    }

    #[test]
    fn and_then_decision_consistent(a in any_sut(), f in dec_fn()) {
        let f2 = f.clone();
        prop_assert_eq!(
            run(lift(a.clone()).and_then_decision(move |x| f2.call(x))),
            a.and_then(|x| f.call(x))
        );
    }

    #[test]
    fn handle_error_with_recovers(e in nec_of(rejection()), f in dec_fn()) {
        let f2 = f.clone();
        let raised: Dtt<i64> = DecisionT::reject(e.clone());
        prop_assert_eq!(
            run(raised.handle_error_with(move |errs| lift(f2.call(errs.len() as i64)))),
            f.call(e.len() as i64)
        );
    }

    #[test]
    fn tail_rec_consistent(a in any::<i64>(), f in dec_fn()) {
        let f2 = f.clone();
        let via_rec = run(Dtt::tail_rec(a, move |x| lift(f2.call(x)).map(ControlFlow::Break)));
        prop_assert_eq!(via_rec, f.call(a));
    }

    #[test]
    fn eq_by_running(a in any_sut()) {
        prop_assert_eq!(run(lift(a.clone())), run(lift(a)));
    }
}

#[test]
fn constructors() {
    assert_eq!(run(Dtt::<i64>::pure(1)), Decision::pure(1));
    assert_eq!(run(Dtt::<()>::unit()), Decision::unit());
    assert_eq!(run(Dtt::<()>::accept(1)), Decision::accept(1));
    assert_eq!(run(Dtt::accept_return(2, 1)), Decision::accept_return(2, 1));
    assert_eq!(
        run(Dtt::<i64>::reject("x".to_string())),
        Decision::reject("x".to_string())
    );
    assert_eq!(run(Dtt::<i64>::validate(Ok(3))), Decision::pure(3));
    assert_eq!(run(Dtt::lift_future(async { 4 })), Decision::pure(4));
    assert_eq!(
        run(Dtt::<i64>::from(Decision::pure(5)).replace(6)),
        Decision::pure(6)
    );
    assert_eq!(block_on(Dtt::<i64>::pure(7).run()), Decision::pure(7));
}

#[test]
fn tail_rec_is_stack_safe() {
    let d: DecisionT<String, i32, u64> = DecisionT::tail_rec(0u64, |i| {
        if i < 100_000 {
            DecisionT::pure(ControlFlow::Continue(i + 1))
        } else {
            DecisionT::pure(ControlFlow::Break(i))
        }
    });
    assert_eq!(block_on(d), Decision::pure(100_000));
}
