//! Port of `action/ActionSuite.scala`.

mod common;

use std::ops::ControlFlow;

use common::*;
use edomata_core::{Action, Decision, ResponseD, nonempty};
use proptest::prelude::*;

type Notif = i64;
type Sut<T> = Action<Rejection, Event, Notif, T>;
type Sut2 = Sut<i64>;
type Resp<T> = ResponseD<Rejection, Event, Notif, T>;

fn run<T>(a: Sut<T>) -> Resp<T> {
    block_on(a)
}

fn action_for(g: BoxedStrategy<Dec<i64>>) -> BoxedStrategy<(Dec<i64>, Vec<Notif>)> {
    (g, prop::collection::vec(any::<Notif>(), 0..=6)).boxed()
}

fn build((d, ns): &(Dec<i64>, Vec<Notif>)) -> Sut2 {
    Action::lift_decision(d.clone()).publish(ns.clone())
}

fn accepted_action() -> BoxedStrategy<(Dec<i64>, Vec<Notif>)> {
    action_for(accepted())
}

fn rejected_action() -> BoxedStrategy<(Dec<i64>, Vec<Notif>)> {
    action_for(rejected())
}

fn not_rejected_action() -> BoxedStrategy<(Dec<i64>, Vec<Notif>)> {
    action_for(not_rejected())
}

fn any_action() -> BoxedStrategy<(Dec<i64>, Vec<Notif>)> {
    action_for(any_sut())
}

#[test]
fn empty_action() {
    let a: Sut<()> = Action::void();
    let res = run(a);
    assert_eq!(res.result, Decision::InDecisive(()));
    assert!(res.notifications.is_empty(), "Non empty notifications!");
}

#[test]
fn notification() {
    let sut = || -> Sut<()> { Action::void().publish([10]) };
    let sut_error = || -> Sut<()> {
        sut()
            .then(Sut::<()>::reject("Some error".to_string()).publish([20]))
            .then(Action::accept(10).publish([10]))
    };

    let res = run(sut());
    assert_eq!(res.result, Decision::InDecisive(()));
    assert_eq!(res.notifications, vec![10]);

    let res2 = run(sut().publish([20]));
    assert_eq!(res2.result, Decision::InDecisive(()));
    assert_eq!(res2.notifications, vec![10, 20]);

    let res3 = run(sut().reset());
    assert_eq!(res3.result, Decision::InDecisive(()));
    assert!(res3.notifications.is_empty(), "Non empty notifications!");

    let res4 = run(sut_error());
    assert_eq!(
        res4.result,
        Decision::Rejected(nonempty!["Some error".to_string()])
    );
    assert_eq!(res4.notifications, vec![20]);
}

#[test]
fn decision() {
    let sut = || -> Sut<i32> { Action::pure(100).publish([10]) };
    let error_sut = || -> Sut<i32> {
        sut().and_then(|i| Action::reject("Some error".to_string()).map(move |()| i))
    };
    let ac1 = |i: i32| -> Sut<()> { Action::accept(nonempty![i + 1, i + 2]) };
    let ac2 = |i: i32| -> Sut<()> { Action::accept(i + 3) };
    let accept_sut = move || -> Sut<i32> {
        sut().and_then(move |i| ac1(i).and_then(move |()| ac2(i)).map(move |()| i * 2))
    };

    let res = run(sut());
    assert_eq!(res.result, Decision::InDecisive(100));
    assert_eq!(res.notifications, vec![10]);

    let error_sut2 = || -> Sut<i32> { accept_sut().then(Action::reject("Some error".to_string())) };

    let res2 = run(error_sut());
    let res3 = run(error_sut2());
    assert_eq!(res2, res3);
    assert_eq!(
        res2.result,
        Decision::Rejected(nonempty!["Some error".to_string()])
    );
    assert!(res2.notifications.is_empty());

    let res4 = run(accept_sut());
    assert_eq!(
        res4.result,
        Decision::Accepted {
            events: nonempty![101, 102, 103],
            result: 200
        }
    );
    assert_eq!(res4.notifications, vec![10]);
}

proptest! {
    #[test]
    fn accumulates_notifications_on_accepted(a in accepted_action(), b in accepted_action()) {
        let c = build(&a).then(build(&b));
        let ares = run(build(&a));
        let bres = run(build(&b));
        let cres = run(c);
        prop_assert_eq!(cres.result, ares.result.then(bres.result));
        let mut ns = ares.notifications;
        ns.extend(bres.notifications);
        prop_assert_eq!(cres.notifications, ns);
    }

    #[test]
    fn rejected_terminates(a in rejected_action(), b in not_rejected_action()) {
        let c = build(&a).then(build(&b));
        let ares = run(build(&a));
        let cres = run(c);
        prop_assert_eq!(cres.result, ares.result);
        prop_assert_eq!(cres.notifications, ares.notifications);
    }

    #[test]
    fn adds_notification_to_rejected_action(a in rejected_action(), ns in prop::collection::vec(any::<Notif>(), 0..=6)) {
        let c = build(&a).publish(ns.clone());
        let ares = run(build(&a));
        let cres = run(c);
        prop_assert_eq!(cres.result, ares.result);
        let mut expected = ares.notifications;
        expected.extend(ns);
        prop_assert_eq!(cres.notifications, expected);
    }

    #[test]
    fn reset_clears_notifications(a in any_action()) {
        let b = build(&a).reset();
        let ares = run(build(&a));
        let bres = run(b);
        prop_assert_eq!(bres.result, ares.result);
        prop_assert!(bres.notifications.is_empty());
    }

    // Monad laws

    #[test]
    fn monad_left_identity(a in any::<i64>(), f in any_action(), k in any::<i64>()) {
        let call = move |x: i64| build(&f).map(move |v| v.wrapping_add(x).wrapping_mul(k));
        prop_assert_eq!(run(Sut2::pure(a).and_then(call.clone())), run(call(a)));
    }

    #[test]
    fn monad_right_identity(a in any_action()) {
        prop_assert_eq!(run(build(&a).and_then(Action::pure)), run(build(&a)));
    }

    #[test]
    fn monad_associativity(a in any_action(), f in any_action(), g in any_action(), k in any::<i64>()) {
        let (f1, g1) = (f.clone(), g.clone());
        let cf = move |x: i64| build(&f1).map(move |v| v.wrapping_add(x).wrapping_mul(k));
        let cg = move |x: i64| build(&g1).map(move |v| v.wrapping_add(x).wrapping_mul(k));
        let (cf2, cg2) = (cf.clone(), cg.clone());
        let lhs = run(build(&a).and_then(cf).and_then(cg));
        let rhs = run(build(&a).and_then(move |x| cf2(x).and_then(cg2)));
        prop_assert_eq!(lhs, rhs);
    }

    #[test]
    fn tail_rec_consistent(a in any::<i64>(), f in any_action(), k in any::<i64>()) {
        let call = move |x: i64| build(&f).map(move |v| v.wrapping_add(x).wrapping_mul(k));
        let call2 = call.clone();
        let via_rec: Sut2 = Action::tail_rec(a, move |x| call2(x).map(ControlFlow::Break));
        prop_assert_eq!(run(via_rec), run(call(a)));
    }

    #[test]
    fn eq_by_running(a in any_action()) {
        prop_assert_eq!(run(build(&a)), run(build(&a)));
    }
}

#[test]
fn constructors() {
    assert_eq!(run(Sut::<()>::unit()), ResponseD::unit());
    assert_eq!(
        run(Sut::<()>::publish_only([1, 2])),
        ResponseD::publish_only([1, 2])
    );
    assert_eq!(run(Sut::<i64>::validate(Ok(1))), ResponseD::pure(1));
    assert_eq!(run(Sut::lift_future(async { 2 })), ResponseD::pure(2));
    assert_eq!(run(Sut::lift(ResponseD::pure(3))), ResponseD::pure(3));
    assert_eq!(run(Sut::<i64>::pure(4).replace(5)), ResponseD::pure(5));
    assert_eq!(block_on(Sut::<i64>::pure(6).run()), ResponseD::pure(6));
}

#[test]
fn tail_rec_accumulates_events_and_notifications() {
    let a: Sut<u32> = Action::tail_rec(0u32, |i| {
        if i < 3 {
            Action::accept(i as i32)
                .publish([i as i64])
                .map(move |()| ControlFlow::Continue(i + 1))
        } else {
            Action::pure(ControlFlow::Break(i)).publish([99])
        }
    });
    let res = run(a);
    assert_eq!(
        res.result,
        Decision::Accepted {
            events: nonempty![0, 1, 2],
            result: 3
        }
    );
    assert_eq!(res.notifications, vec![0, 1, 2, 99]);
}
