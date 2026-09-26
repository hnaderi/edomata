//! Port of `responset/ResponseTLaws.scala` and
//! `responset/ResponseDecisionSuite.scala` (`ResponseDecisionSuite` and
//! `ResponseEitherNecSuite`).
//!
//! The Cats `Traverse` law checks are not ported: `ResponseT` has no
//! `Traverse` abstraction in Rust; `map` covers the functor part and the
//! monad laws are checked below.

mod common;

use std::fmt::Debug;

use common::*;
use edomata_core::{Decision, NonEmpty, RaiseError, ResponseD, ResponseE, ResponseT};
use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};

type App<Res> = ResponseT<Res, Notification>;

/// A deterministic function `i64 -> ResponseT<Res, N>` built from a template.
#[derive(Clone, Debug)]
struct RespFn<Res> {
    template: Res,
    notifications: Vec<Notification>,
    k: i64,
}

impl<Res> RespFn<Res>
where
    Res: RaiseError<Output = i64, WithOutput<i64> = Res> + Clone,
{
    fn call(&self, a: i64) -> App<Res> {
        let k = self.k;
        ResponseT::new(
            self.template
                .clone()
                .map(|v| v.wrapping_add(a).wrapping_mul(k)),
            self.notifications.clone(),
        )
    }
}

fn run<S: Strategy>(strategy: S, test: impl Fn(S::Value) -> Result<(), TestCaseError>) {
    let mut runner = TestRunner::new(Config::with_cases(128));
    runner.run(&strategy, test).unwrap();
}

fn check_laws<Res>(rejected: BoxedStrategy<Res>, not_rejected: BoxedStrategy<Res>)
where
    Res: RaiseError<Output = i64, Rejection = Rejection, WithOutput<i64> = Res>
        + Clone
        + PartialEq
        + Debug
        + 'static,
{
    let any_sut = || prop_oneof![rejected.clone(), not_rejected.clone()].boxed();
    let response = |d: Res, ns: Vec<Notification>| App::new(d, ns);
    let resp_fn = || {
        (any_sut(), notifications(), any::<i64>())
            .prop_map(|(template, notifications, k)| RespFn {
                template,
                notifications,
                k,
            })
            .boxed()
    };

    // Accumulates on accept
    run(
        (
            not_rejected.clone(),
            notifications(),
            not_rejected.clone(),
            notifications(),
        ),
        |(a1, n1, a2, n2)| {
            let r1 = response(a1.clone(), n1.clone());
            let r2 = response(a2.clone(), n2.clone());
            let r3 = r1.then::<i64>(r2);
            let mut ns = n1;
            ns.extend(n2);
            prop_assert_eq!(r3, response(a1.and_then::<i64, _>(|_| a2), ns));
            Ok(())
        },
    );

    // Resets on rejection
    run(
        (
            not_rejected.clone(),
            notifications(),
            rejected.clone(),
            notifications(),
        ),
        |(a1, n1, a2, n2)| {
            let r1 = response(a1.clone(), n1);
            let r2 = response(a2.clone(), n2.clone());
            let r3 = r1.then::<i64>(r2);
            prop_assert_eq!(r3.clone(), response(a1.and_then::<i64, _>(|_| a2), n2));
            prop_assert!(r3.result.is_error());
            Ok(())
        },
    );

    // Rejected does not change
    run(
        (
            rejected.clone(),
            notifications(),
            any_sut(),
            notifications(),
        ),
        |(a1, n1, a2, n2)| {
            let r1 = response(a1, n1);
            let r2 = response(a2, n2);
            let r3 = r1.clone().then::<i64>(r2);
            prop_assert_eq!(r3.clone(), r1);
            prop_assert!(r3.result.is_error());
            Ok(())
        },
    );

    // Publish on rejection
    run(
        (rejected.clone(), notifications(), notifications()),
        |(a1, n1, n2)| {
            let r1 = response(a1, n1);
            let r2 = r1.clone().publish_on_rejection(n2.clone());
            let mut ns = r1.notifications.clone();
            ns.extend(n2);
            prop_assert_eq!(r2, ResponseT::new(r1.result, ns));
            Ok(())
        },
    );

    // Publish on rejection does nothing when not rejected
    run(
        (not_rejected.clone(), notifications(), notifications()),
        |(a1, n1, n2)| {
            let r1 = response(a1, n1);
            prop_assert_eq!(r1.clone().publish_on_rejection(n2), r1);
            Ok(())
        },
    );

    // Publish adds notifications
    run(
        (any_sut(), notifications(), notifications()),
        |(a1, n1, n2)| {
            let r1 = response(a1.clone(), n1.clone());
            let r2 = r1.publish(n2.clone());
            let mut ns = n1;
            ns.extend(n2);
            prop_assert_eq!(r2, response(a1, ns));
            Ok(())
        },
    );

    // Reset clears notifications
    run((any_sut(), notifications()), |(a1, n1)| {
        let r1 = response(a1, n1);
        let r2 = r1.clone().reset();
        prop_assert!(r2.notifications.is_empty());
        prop_assert_eq!(r2.result, r1.result);
        Ok(())
    });

    // Monad laws: left identity
    run((any::<i64>(), resp_fn()), |(a, f)| {
        prop_assert_eq!(
            App::<Res>::pure(a).and_then::<i64, _>(|x| f.call(x)),
            f.call(a)
        );
        Ok(())
    });

    // Monad laws: right identity
    run((any_sut(), notifications()), |(a1, n1)| {
        let r1 = response(a1, n1);
        prop_assert_eq!(r1.clone().and_then::<i64, _>(App::<Res>::pure), r1);
        Ok(())
    });

    // Monad laws: associativity
    run(
        (any_sut(), notifications(), resp_fn(), resp_fn()),
        |(a1, n1, f, g)| {
            let r1 = response(a1, n1);
            let lhs = r1
                .clone()
                .and_then::<i64, _>(|x| f.call(x))
                .and_then::<i64, _>(|x| g.call(x));
            let rhs = r1.and_then::<i64, _>(|x| f.call(x).and_then::<i64, _>(|y| g.call(y)));
            prop_assert_eq!(lhs, rhs);
            Ok(())
        },
    );

    // Functor: map consistent with and_then + pure
    run((any_sut(), notifications(), any::<i64>()), |(a1, n1, k)| {
        let r1 = response(a1, n1);
        prop_assert_eq!(
            r1.clone().map(|x| x.wrapping_mul(k)),
            r1.and_then::<i64, _>(|x| App::<Res>::pure(x.wrapping_mul(k)))
        );
        Ok(())
    });

    // MonadError: raise then handle
    run((nec_of(rejection()), resp_fn()), |(e, f)| {
        let raised = App::<Res>::reject(e.clone());
        prop_assert_eq!(
            raised.handle_error_with(|errs| f.call(errs.len() as i64)),
            f.call(e.len() as i64)
        );
        Ok(())
    });

    // MonadError: pure is not handled
    run((any::<i64>(), resp_fn()), |(a, f)| {
        let p = App::<Res>::pure(a);
        prop_assert_eq!(
            p.clone()
                .handle_error_with(|errs| f.call(errs.len() as i64)),
            p
        );
        Ok(())
    });

    // tailRecM consistency
    run((any::<i64>(), resp_fn()), |(a, f)| {
        let via_rec: App<Res> =
            App::<Res>::tail_rec(a, |x| f.call(x).map(std::ops::ControlFlow::Break));
        prop_assert_eq!(via_rec, f.call(a));
        Ok(())
    });

    // Eq laws
    run(
        (any_sut(), notifications(), any_sut(), notifications()),
        |(a1, n1, a2, n2)| {
            let r1 = response(a1, n1);
            let r2 = response(a2, n2);
            prop_assert_eq!(r1.clone(), r1.clone());
            prop_assert_eq!(r1 == r2, r2 == r1);
            Ok(())
        },
    );
}

#[test]
fn response_decision_laws() {
    check_laws::<Decision<Rejection, Event, i64>>(rejected(), not_rejected());
}

#[test]
fn response_result_nec_laws() {
    check_laws::<Result<i64, NonEmpty<Rejection>>>(
        nec_of(rejection()).prop_map(Err).boxed(),
        any::<i64>().prop_map(Ok).boxed(),
    );
}

#[test]
fn response_d_constructors() {
    type R = ResponseD<String, i32, Notification, ()>;
    assert_eq!(R::unit(), ResponseT::lift(Decision::unit()));
    assert_eq!(R::accept(1), ResponseT::lift(Decision::accept(1)));
    assert_eq!(
        ResponseD::<String, i32, Notification, i32>::accept_return(2, 1),
        ResponseT::lift(Decision::accept_return(2, 1))
    );
    assert_eq!(
        R::publish_only([Notification("a".into())]),
        ResponseT::new(Decision::unit(), [Notification("a".into())])
    );
    assert_eq!(
        R::reject("x".to_string()),
        ResponseT::lift(Decision::reject("x".to_string()))
    );
    assert_eq!(
        ResponseD::<String, i32, Notification, i32>::validate(Ok(1)),
        ResponseT::lift(Decision::pure(1))
    );
    assert_eq!(
        ResponseD::<String, i32, Notification, i32>::decide(Decision::pure(1)),
        ResponseT::lift(Decision::pure(1))
    );
    assert!(R::reject("x".to_string()).is_rejected());
}

#[test]
fn response_e_constructors() {
    type R = ResponseE<String, Notification, ()>;
    assert_eq!(R::unit(), ResponseT::lift(Ok(())));
    assert_eq!(R::pure(()), ResponseT::lift(Ok(())));
    assert_eq!(
        R::publish_only([Notification("a".into())]),
        ResponseT::new(Ok(()), [Notification("a".into())])
    );
    assert_eq!(
        R::reject("x".to_string()),
        ResponseT::lift(Err(NonEmpty::new("x".to_string())))
    );
    assert_eq!(
        ResponseE::<String, Notification, i32>::validate(Err(NonEmpty::new("x".to_string()))),
        ResponseT::lift(Err(NonEmpty::new("x".to_string())))
    );
}

#[test]
fn publish_on_rejection_with_receives_reasons() {
    let r: ResponseD<String, i32, String, ()> = ResponseT::reject("bad".to_string());
    let r = r.publish_on_rejection_with(|errs| {
        errs.iter()
            .map(|e| format!("failed: {e}"))
            .collect::<Vec<_>>()
    });
    assert_eq!(r.notifications, vec!["failed: bad".to_string()]);
}

#[test]
fn map_notifications_changes_type() {
    let r: ResponseD<String, i32, i32, ()> = ResponseD::publish_only([1, 2]);
    let r = r.map_notifications(|n| n.to_string());
    assert_eq!(r.notifications, vec!["1".to_string(), "2".to_string()]);
}
