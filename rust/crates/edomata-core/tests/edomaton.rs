//! Port of `edomaton/EdomatonSuite.scala`: Monad, Contravariant and Eq laws,
//! checked by running programs on the exhaustive inputs `1, 2, 3` (the
//! Scala suite's `ExhaustiveCheck[Int]`).

mod common;

use common::*;
use edomata_core::{Decision, Edomaton, ResponseD, nonempty};
use proptest::prelude::*;

type Env = i32;
type App<T> = Edomaton<Env, Rejection, Event, Notification, T>;
type Resp<T> = ResponseD<Rejection, Event, Notification, T>;

const ENVS: [Env; 3] = [1, 2, 3];

/// Runs a program on every exhaustive input.
fn run_all<T>(app: &App<T>) -> Vec<Resp<T>>
where
    T: Send + 'static,
{
    ENVS.iter().map(|env| block_on(app.run(*env))).collect()
}

/// Template of an arbitrary edomaton: (notifications, decision).
type Template = (Vec<Notification>, Dec<i64>);

fn template() -> BoxedStrategy<Template> {
    (notifications(), any_sut()).boxed()
}

fn build((ns, d): &Template) -> App<i64> {
    let ns = ns.clone();
    let d = d.clone();
    Edomaton::from_fn(move |env: Env| {
        ResponseD::new(d.clone().map(|v| v.wrapping_add(env as i64)), ns.clone())
    })
}

/// An arbitrary function `i64 -> App<i64>` derived from a template.
fn build_fn(t: Template, k: i64) -> impl Fn(i64) -> App<i64> + Clone + use<> {
    move |x: i64| build(&t).map(move |v| v.wrapping_add(x).wrapping_mul(k))
}

proptest! {
    // Monad laws

    #[test]
    fn monad_left_identity(a in any::<i64>(), f in template(), k in any::<i64>()) {
        let call = build_fn(f, k);
        let lhs: App<i64> = Edomaton::pure(a).and_then(call.clone());
        prop_assert_eq!(run_all(&lhs), run_all(&call(a)));
    }

    #[test]
    fn monad_right_identity(a in template()) {
        let lhs = build(&a).and_then(Edomaton::pure);
        prop_assert_eq!(run_all(&lhs), run_all(&build(&a)));
    }

    #[test]
    fn monad_associativity(a in template(), f in template(), g in template(), k in any::<i64>()) {
        let (cf, cg) = (build_fn(f, k), build_fn(g, k));
        let (cf2, cg2) = (cf.clone(), cg.clone());
        let lhs = build(&a).and_then(cf).and_then(cg);
        let rhs = build(&a).and_then(move |x| cf2(x).and_then(cg2.clone()));
        prop_assert_eq!(run_all(&lhs), run_all(&rhs));
    }

    #[test]
    fn functor_identity(a in template()) {
        prop_assert_eq!(run_all(&build(&a).map(|x| x)), run_all(&build(&a)));
    }

    #[test]
    fn functor_composition(a in template(), f in int_fn(), g in int_fn()) {
        let lhs = build(&a).map(move |x| f.call(x)).map(move |x| g.call(x));
        let rhs = build(&a).map(move |x| g.call(f.call(x)));
        prop_assert_eq!(run_all(&lhs), run_all(&rhs));
    }

    #[test]
    fn map_consistent_with_and_then(a in template(), f in int_fn()) {
        let lhs = build(&a).map(move |x| f.call(x));
        let rhs = build(&a).and_then(move |x| Edomaton::pure(f.call(x)));
        prop_assert_eq!(run_all(&lhs), run_all(&rhs));
    }

    // Contravariant laws

    #[test]
    fn contravariant_identity(a in template()) {
        prop_assert_eq!(run_all(&build(&a).contramap(|e: Env| e)), run_all(&build(&a)));
    }

    #[test]
    fn contravariant_composition(a in template(), f in any::<i32>(), g in any::<i32>()) {
        let lhs = build(&a).contramap(move |e: Env| e.wrapping_add(f)).contramap(move |e: Env| e.wrapping_mul(g));
        let rhs = build(&a).contramap(move |e: Env| e.wrapping_mul(g).wrapping_add(f));
        prop_assert_eq!(run_all(&lhs), run_all(&rhs));
    }

    // Eq laws

    #[test]
    fn eq_by_running(a in template()) {
        prop_assert_eq!(run_all(&build(&a)), run_all(&build(&a)));
    }

    // Response semantics through and_then

    #[test]
    fn and_then_matches_response_then(a in template(), b in template()) {
        let c = build(&a).then(build(&b));
        for (env, out) in ENVS.iter().zip(run_all(&c)) {
            let ra = block_on(build(&a).run(*env));
            let rb = block_on(build(&b).run(*env));
            prop_assert_eq!(out, ra.then(rb));
        }
    }

    #[test]
    fn handle_error_with_recovers(e in nec_of(rejection()), a in template()) {
        let rejected: App<i64> = Edomaton::reject(e.clone());
        let a2 = a.clone();
        let recovered = rejected.handle_error_with(move |errs| build(&a2).map(move |v| v.wrapping_add(errs.len() as i64)));
        let expected = build(&a).map(move |v| v.wrapping_add(e.len() as i64));
        prop_assert_eq!(run_all(&recovered), run_all(&expected));
    }
}

#[test]
fn constructors_and_combinators() {
    let env = 7;
    let run = |app: &App<i64>| block_on(app.run(env));
    let run_unit = |app: &App<()>| block_on(app.run(env));

    assert_eq!(run(&Edomaton::pure(1)), ResponseD::pure(1));
    assert_eq!(run_unit(&Edomaton::unit()), ResponseD::unit());
    assert_eq!(block_on(App::<Env>::read().run(env)), ResponseD::pure(env));
    assert_eq!(run(&Edomaton::eval(|| async { 2 })), ResponseD::pure(2));
    assert_eq!(
        run(&Edomaton::run_with(|e: Env| async move { e as i64 * 2 })),
        ResponseD::pure(14)
    );
    assert_eq!(
        run(&Edomaton::map_input(|e: Env| e as i64)),
        ResponseD::pure(7)
    );
    assert_eq!(run(&Edomaton::lift(ResponseD::pure(3))), ResponseD::pure(3));
    assert_eq!(
        run(&Edomaton::lift_future(|| async { ResponseD::pure(4) })),
        ResponseD::pure(4)
    );
    assert_eq!(
        run(&Edomaton::reject("x".to_string())),
        ResponseD::reject("x".to_string())
    );
    assert_eq!(
        run(&Edomaton::decide(Decision::accept_return(5, 1))),
        ResponseD::accept_return(5, 1)
    );
    assert_eq!(run(&Edomaton::validate(Ok(6))), ResponseD::pure(6));
    assert_eq!(
        run(&Edomaton::from_option(None, "none".to_string())),
        ResponseD::reject("none".to_string())
    );
    assert_eq!(run(&Edomaton::from_result(Ok(8))), ResponseD::pure(8));
    assert_eq!(
        run(&Edomaton::from_result_nec(Err(nonempty!["e".to_string()]))),
        ResponseD::reject("e".to_string())
    );
    assert_eq!(
        run_unit(&Edomaton::accept(nonempty![1, 2])),
        ResponseD::accept(nonempty![1, 2])
    );
    assert_eq!(
        run_unit(&Edomaton::publish_only([Notification("n".into())])),
        ResponseD::publish_only([Notification("n".into())])
    );

    // publish / reset / publish_on_rejection
    let n = Notification("n".into());
    let p: App<i64> = Edomaton::pure(1).publish([n.clone()]);
    assert_eq!(run(&p).notifications, vec![n.clone()]);
    assert_eq!(
        run(&p.clone().reset()).notifications,
        Vec::<Notification>::new()
    );
    assert_eq!(
        run(&p.clone().publish_on_rejection([Notification("r".into())])).notifications,
        vec![n.clone()]
    );
    let r: App<i64> =
        Edomaton::reject("x".to_string()).publish_on_rejection([Notification("r".into())]);
    assert_eq!(run(&r).notifications, vec![Notification("r".into())]);
    let r2: App<i64> = Edomaton::reject("x".to_string()).publish_on_rejection_with(|errs| {
        errs.iter()
            .map(|e| Notification(e.clone()))
            .collect::<Vec<_>>()
    });
    assert_eq!(run(&r2).notifications, vec![Notification("x".into())]);

    // eval_map / eval_tap / eval_with / decide_with / replace / void / transform
    assert_eq!(
        run(&Edomaton::pure(1).eval_map(|x| async move { x + 1 })),
        ResponseD::pure(2)
    );
    assert_eq!(
        run(&Edomaton::pure(1).eval_tap(|_| async {})),
        ResponseD::pure(1)
    );
    assert_eq!(
        run(&Edomaton::pure(1).eval_with(|| async {})),
        ResponseD::pure(1)
    );
    assert_eq!(
        run(&Edomaton::pure(1).decide_with(|x| Decision::accept_return(x * 10, 1))),
        ResponseD::accept_return(10, 1)
    );
    assert_eq!(run(&Edomaton::pure(1).replace(9)), ResponseD::pure(9));
    assert_eq!(run_unit(&Edomaton::pure(1).void()), ResponseD::unit());
    assert_eq!(
        run(&Edomaton::pure(1).transform(|r: Resp<i64>| r.map(|x| x + 100))),
        ResponseD::pure(101)
    );

    // eval_map is not run on rejection
    let rejected: App<i64> = App::<i64>::reject("x".to_string())
        .eval_map(|_: i64| async { panic!("eval_map must not run on rejection") });
    assert_eq!(run(&rejected), ResponseD::reject("x".to_string()));

    // Programs are reusable
    let counter: App<i64> = Edomaton::map_input(|e: Env| e as i64);
    assert_eq!(block_on(counter.run(1)), ResponseD::pure(1));
    assert_eq!(block_on(counter.run(2)), ResponseD::pure(2));
}
