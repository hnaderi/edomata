//! Port of `stomaton/StomatonSuite.scala` (MonadError, Contravariant and Eq
//! laws) and `stomaton/StomatonConstructorSuite.scala`.

mod common;

use common::*;
use edomata_core::{NonEmpty, ResponseE, Stomaton, nonempty};
use proptest::prelude::*;

type State = i32;
type Env = i32;
type AppS<Env, T> = Stomaton<Env, State, Rejection, Event, T>;
type App<T> = AppS<Env, T>;
type Resp<T> = ResponseE<Rejection, Event, (State, T)>;

const ENVS: [Env; 3] = [1, 2, 3];
const STATES: [State; 3] = [0, 5, -3];

fn run_all<T: Send + 'static>(app: &App<T>) -> Vec<Resp<T>> {
    let mut out = Vec::new();
    for env in ENVS {
        for s in STATES {
            out.push(block_on(app.run(env, s)));
        }
    }
    out
}

/// Arbitrary stomaton, as in Scala: `Stomaton.decide(Either[NonEmptyChain[R], T])`.
type Template = Result<i64, NonEmpty<Rejection>>;

fn template() -> BoxedStrategy<Template> {
    prop_oneof![nec_of(rejection()).prop_map(Err), any::<i64>().prop_map(Ok),].boxed()
}

fn build(t: &Template) -> App<i64> {
    Stomaton::decide(t.clone())
}

fn build_fn(t: Template, k: i64) -> impl Fn(i64) -> App<i64> + Clone + use<> {
    move |x: i64| build(&t).map(move |v| v.wrapping_add(x).wrapping_mul(k))
}

proptest! {
    #[test]
    fn monad_left_identity(a in any::<i64>(), f in template(), k in any::<i64>()) {
        let call = build_fn(f, k);
        let lhs: App<i64> = Stomaton::pure(a).and_then(call.clone());
        prop_assert_eq!(run_all(&lhs), run_all(&call(a)));
    }

    #[test]
    fn monad_right_identity(a in template()) {
        prop_assert_eq!(run_all(&build(&a).and_then(Stomaton::pure)), run_all(&build(&a)));
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
    fn functor_composition(a in template(), f in int_fn(), g in int_fn()) {
        let lhs = build(&a).map(move |x| f.call(x)).map(move |x| g.call(x));
        let rhs = build(&a).map(move |x| g.call(f.call(x)));
        prop_assert_eq!(run_all(&lhs), run_all(&rhs));
    }

    #[test]
    fn raise_error_handled(e in nec_of(rejection()), f in template(), k in any::<i64>()) {
        let call = build_fn(f, k);
        let raised: App<i64> = Stomaton::decide(Err(e.clone()));
        let call2 = call.clone();
        let lhs = raised.handle_error_with(move |errs| call2(errs.len() as i64));
        prop_assert_eq!(run_all(&lhs), run_all(&call(e.len() as i64)));
    }

    #[test]
    fn pure_is_not_handled(a in any::<i64>(), f in template(), k in any::<i64>()) {
        let call = build_fn(f, k);
        let p: App<i64> = Stomaton::pure(a);
        let lhs = p.clone().handle_error_with(move |errs| call(errs.len() as i64));
        prop_assert_eq!(run_all(&lhs), run_all(&p));
    }

    #[test]
    fn raise_error_short_circuits(e in nec_of(rejection()), f in template(), k in any::<i64>()) {
        let call = build_fn(f, k);
        let raised: App<i64> = Stomaton::decide(Err(e.clone()));
        let lhs = raised.clone().and_then(call);
        prop_assert_eq!(run_all(&lhs), run_all(&raised));
    }

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

    #[test]
    fn eq_by_running(a in template()) {
        prop_assert_eq!(run_all(&build(&a)), run_all(&build(&a)));
    }

    #[test]
    fn notifications_accumulate_across_and_then(n1 in prop::collection::vec(any::<Event>(), 0..4), n2 in prop::collection::vec(any::<Event>(), 0..4), t in template()) {
        let a: App<()> = Stomaton::publish_only(n1.clone());
        let b: App<i64> = build(&t).publish(n2.clone());
        let out = block_on(a.then(b).run(1, 0));
        let mut expected = n1;
        expected.extend(n2);
        prop_assert_eq!(out.notifications, expected);
        prop_assert_eq!(out.result, t.map(|v| (0, v)));
    }
}

// --- StomatonConstructorSuite ------------------------------------------

#[test]
fn pure() {
    let a: App<&str> = Stomaton::pure("hello");
    assert_eq!(block_on(a.run(0, 0)), ResponseE::lift(Ok((0, "hello"))));
}

#[test]
fn set_state() {
    let a: App<()> = Stomaton::set(2);
    assert_eq!(block_on(a.run(0, 0)), ResponseE::lift(Ok((2, ()))));
}

#[test]
fn context() {
    let a: App<Env> = Stomaton::context();
    assert_eq!(block_on(a.run(10, 0)), ResponseE::lift(Ok((0, 10))));
}

#[test]
fn state() {
    let a: App<State> = Stomaton::state();
    assert_eq!(block_on(a.run(10, 0)), ResponseE::lift(Ok((0, 0))));
}

#[test]
fn modify() {
    let a: App<State> = Stomaton::modify(|s| s + 2);
    assert_eq!(block_on(a.run(10, 0)), ResponseE::lift(Ok((2, 2))));
}

#[test]
fn modify_s() {
    let a: App<State> = Stomaton::modify_s(|i| Ok(i + 2));
    assert_eq!(block_on(a.run(10, 0)), ResponseE::lift(Ok((2, 2))));
    let b: App<State> = Stomaton::modify_s(|_| Err(NonEmpty::new(String::new())));
    assert_eq!(
        block_on(b.run(10, 0)),
        ResponseE::lift(Err(nonempty![String::new()]))
    );
}

#[test]
fn publish() {
    let a: App<()> = Stomaton::publish_only([1, 2, 3]);
    assert_eq!(
        block_on(a.run(10, 0)),
        ResponseE::new(Ok((0, ())), [1, 2, 3])
    );
}

#[test]
fn other_constructors_and_combinators() {
    let run = |a: &App<i64>| block_on(a.run(10, 0));
    assert_eq!(
        block_on(App::<()>::unit().run(1, 2)),
        ResponseE::lift(Ok((2, ())))
    );
    assert_eq!(
        run(&Stomaton::eval(|| async { 1 })),
        ResponseE::lift(Ok((0, 1)))
    );
    assert_eq!(
        run(&Stomaton::run_with(|e: Env| async move { e as i64 })),
        ResponseE::lift(Ok((0, 10)))
    );
    assert_eq!(
        run(&Stomaton::map_input(|e: Env| e as i64 + 1)),
        ResponseE::lift(Ok((0, 11)))
    );
    assert_eq!(
        run(&Stomaton::decide_with(|s: &State| Ok(*s as i64 + 1))),
        ResponseE::lift(Ok((0, 1)))
    );
    assert_eq!(
        run(&Stomaton::decide_s(|s| Ok(s + 1)).map(|s| s as i64)),
        ResponseE::lift(Ok((1, 1)))
    );
    assert_eq!(
        run(&Stomaton::reject("x".to_string())),
        ResponseE::reject("x".to_string())
    );
    assert_eq!(run(&Stomaton::validate(Ok(2))), ResponseE::lift(Ok((0, 2))));
    assert_eq!(
        run(&Stomaton::from_option(None, "none".to_string())),
        ResponseE::reject("none".to_string())
    );
    assert_eq!(
        run(&Stomaton::from_result(Ok(3))),
        ResponseE::lift(Ok((0, 3)))
    );
    assert_eq!(
        run(&Stomaton::from_result_nec(Err(nonempty!["e".to_string()]))),
        ResponseE::reject("e".to_string())
    );

    // combinators
    assert_eq!(
        run(&Stomaton::pure(1).modify_state(|s| s + 7)),
        ResponseE::lift(Ok((7, 1)))
    );
    assert_eq!(
        run(&Stomaton::pure(1).set_state(4)),
        ResponseE::lift(Ok((4, 1)))
    );
    assert_eq!(
        run(&Stomaton::pure(1)
            .decide_state(|s| Ok(s + 3))
            .map(|s| s as i64)),
        ResponseE::lift(Ok((3, 3)))
    );
    assert_eq!(
        run(&Stomaton::pure(1).decide_output(|a| Ok(a * 5))),
        ResponseE::lift(Ok((0, 5)))
    );
    assert_eq!(
        run(&Stomaton::pure(1).decide_output(|_| Err(nonempty!["bad".to_string()]))),
        ResponseE::reject("bad".to_string())
    );
    assert_eq!(
        run(&Stomaton::pure(1).replace(2)),
        ResponseE::lift(Ok((0, 2)))
    );
    assert_eq!(
        block_on(App::<i64>::pure(1).void().run(1, 0)),
        ResponseE::lift(Ok((0, ())))
    );
    assert_eq!(run(&Stomaton::pure(1).publish([9])).notifications, vec![9]);
    let rejected: App<i64> = Stomaton::reject("x".to_string());
    assert_eq!(
        run(&rejected.clone().publish_on_rejection([8])).notifications,
        vec![8]
    );
    assert_eq!(
        run(&rejected
            .clone()
            .publish_on_rejection_with(|errs| vec![errs.len() as i32]))
        .notifications,
        vec![1]
    );
    assert_eq!(
        run(&Stomaton::pure(1).publish_on_rejection([8])).notifications,
        Vec::<i32>::new()
    );
    assert_eq!(
        run(&Stomaton::pure(1).transform(|r: Resp<i64>| r.map(|(s, a)| (s + 1, a + 1)))),
        ResponseE::lift(Ok((1, 2)))
    );

    // handle_error_with runs the handler with the original state
    let recovered = Stomaton::<Env, State, Rejection, Event, ()>::set(99)
        .then(Stomaton::<Env, State, Rejection, Event, i64>::reject(
            "x".to_string(),
        ))
        .handle_error_with(|_| Stomaton::state().map(|s| s as i64));
    assert_eq!(run(&recovered), ResponseE::lift(Ok((0, 0))));

    // and_then threads state
    let threaded: App<State> =
        Stomaton::modify(|s| s + 1).and_then(|_| Stomaton::modify(|s| s * 10));
    assert_eq!(block_on(threaded.run(0, 1)), ResponseE::lift(Ok((20, 20))));
}
