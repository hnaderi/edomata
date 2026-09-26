//! [`Edomaton`]: the event-driven automaton.

use std::fmt;
use std::future::Future;
use std::sync::Arc;

use crate::{BoxFuture, Decision, NonEmpty, ResponseD};

/// The reusable program behind an [`Edomaton`].
type RunFn<Env, R, E, N, A> =
    dyn Fn(Env) -> BoxFuture<'static, ResponseD<R, E, N, A>> + Send + Sync;

/// An event-driven automaton: a reusable, asynchronous program that reads an
/// input (`Env`), decides on a state transition by accepting events or
/// rejecting, and optionally publishes notifications.
///
/// # Type parameters
///
/// * `Env` – input type, usually a [`RequestContext`](crate::RequestContext)
/// * `R` – rejection type
/// * `E` – internal (domain) event type
/// * `N` – notification type, a.k.a. external or integration event
/// * `A` – output type
///
/// Edomata programs are values: they can be stored, cloned and run many
/// times. Composition uses [`Edomaton::and_then`] (or [`Edomaton::then`]),
/// with the same accumulation and short-circuit semantics as
/// [`Decision`].
///
/// ```
/// use edomata_core::{Decision, Edomaton, nonempty};
///
/// let deposit: Edomaton<i64, &str, i64, &str, ()> = Edomaton::read().and_then(|amount| {
///     if amount > 0 {
///         Edomaton::decide(Decision::accept(amount)).publish(["deposited"])
///     } else {
///         Edomaton::reject("amount must be positive")
///     }
/// });
///
/// let ok = futures::executor::block_on(deposit.run(10));
/// assert_eq!(ok.result, Decision::Accepted { events: nonempty![10], result: () });
/// assert_eq!(ok.notifications, vec!["deposited"]);
///
/// let ko = futures::executor::block_on(deposit.run(-1));
/// assert!(ko.result.is_rejected());
/// ```
pub struct Edomaton<Env, R, E, N, A> {
    run: Arc<RunFn<Env, R, E, N, A>>,
}

impl<Env, R, E, N, A> Clone for Edomaton<Env, R, E, N, A> {
    fn clone(&self) -> Self {
        Self {
            run: Arc::clone(&self.run),
        }
    }
}

impl<Env, R, E, N, A> fmt::Debug for Edomaton<Env, R, E, N, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Edomaton(<program>)")
    }
}

impl<Env, R, E, N, A> Edomaton<Env, R, E, N, A>
where
    Env: Send + 'static,
    R: Send + 'static,
    E: Send + 'static,
    N: Send + 'static,
    A: Send + 'static,
{
    // ---------------------------------------------------------------------
    // Constructors
    // ---------------------------------------------------------------------

    /// Constructs an edomaton from an asynchronous function of its input.
    pub fn new<F, Fut>(f: F) -> Self
    where
        F: Fn(Env) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ResponseD<R, E, N, A>> + Send + 'static,
    {
        Self {
            run: Arc::new(move |env| Box::pin(f(env))),
        }
    }

    /// Constructs an edomaton from a pure function of its input.
    pub fn from_fn<F>(f: F) -> Self
    where
        F: Fn(Env) -> ResponseD<R, E, N, A> + Send + Sync + 'static,
    {
        Self::new(move |env| std::future::ready(f(env)))
    }

    /// Constructs an edomaton that outputs a pure value.
    pub fn pure(a: A) -> Self
    where
        A: Clone + Sync,
    {
        Self::from_fn(move |_| ResponseD::pure(a.clone()))
    }

    /// Constructs an edomaton from an effect factory that yields a response.
    pub fn lift_future<F, Fut>(f: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ResponseD<R, E, N, A>> + Send + 'static,
    {
        Self::new(move |_| f())
    }

    /// Constructs an edomaton with the given response.
    pub fn lift(response: ResponseD<R, E, N, A>) -> Self
    where
        ResponseD<R, E, N, A>: Clone + Sync,
    {
        Self::from_fn(move |_| response.clone())
    }

    /// Constructs an edomaton that evaluates an effect, whose output becomes
    /// the program output.
    pub fn eval<F, Fut>(f: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = A> + Send + 'static,
    {
        Self::new(move |_| {
            let fut = f();
            async move { ResponseD::pure(fut.await) }
        })
    }

    /// Constructs an edomaton that runs an effect using its input.
    pub fn run_with<F, Fut>(f: F) -> Self
    where
        F: Fn(Env) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = A> + Send + 'static,
    {
        Self::new(move |env| {
            let fut = f(env);
            async move { ResponseD::pure(fut.await) }
        })
    }

    /// Constructs an edomaton that outputs a pure function of its input.
    pub fn map_input<F>(f: F) -> Self
    where
        F: Fn(Env) -> A + Send + Sync + 'static,
    {
        Self::from_fn(move |env| ResponseD::pure(f(env)))
    }

    /// Constructs an edomaton that rejects with the given reasons.
    pub fn reject(reasons: impl Into<NonEmpty<R>>) -> Self
    where
        R: Clone + Sync,
    {
        let reasons = reasons.into();
        Self::from_fn(move |_| ResponseD::reject(reasons.clone()))
    }

    /// Constructs an edomaton that decides the given decision.
    pub fn decide(decision: Decision<R, E, A>) -> Self
    where
        Decision<R, E, A>: Clone + Sync,
    {
        Self::from_fn(move |_| ResponseD::lift(decision.clone()))
    }

    /// Constructs an edomaton from a validation.
    pub fn validate(validation: Result<A, NonEmpty<R>>) -> Self
    where
        Decision<R, E, A>: Clone + Sync,
    {
        Self::decide(Decision::validate(validation))
    }

    /// Constructs an edomaton from an optional value, that outputs the value
    /// if it exists or rejects otherwise.
    pub fn from_option(opt: Option<A>, or_else: impl Into<NonEmpty<R>>) -> Self
    where
        Decision<R, E, A>: Clone + Sync,
    {
        Self::decide(Decision::from_option(opt, or_else))
    }

    /// Constructs an edomaton that either outputs a value or rejects with a
    /// single reason.
    pub fn from_result(result: Result<A, R>) -> Self
    where
        Decision<R, E, A>: Clone + Sync,
    {
        Self::decide(Decision::from_result(result))
    }

    /// Constructs an edomaton that either outputs a value or rejects with
    /// several reasons.
    pub fn from_result_nec(result: Result<A, NonEmpty<R>>) -> Self
    where
        Decision<R, E, A>: Clone + Sync,
    {
        Self::decide(Decision::validate(result))
    }

    // ---------------------------------------------------------------------
    // Running
    // ---------------------------------------------------------------------

    /// Runs this edomaton with the given input.
    pub fn run(&self, env: Env) -> BoxFuture<'static, ResponseD<R, E, N, A>> {
        (self.run)(env)
    }

    // ---------------------------------------------------------------------
    // Combinators
    // ---------------------------------------------------------------------

    /// Transforms the underlying response.
    pub fn transform<R2, E2, N2, B, F>(self, f: F) -> Edomaton<Env, R2, E2, N2, B>
    where
        R2: Send + 'static,
        E2: Send + 'static,
        N2: Send + 'static,
        B: Send + 'static,
        F: Fn(ResponseD<R, E, N, A>) -> ResponseD<R2, E2, N2, B> + Send + Sync + 'static,
    {
        let f = Arc::new(f);
        Edomaton::new(move |env| {
            let fut = self.run(env);
            let f = Arc::clone(&f);
            async move { f(fut.await) }
        })
    }

    /// Maps the output.
    pub fn map<B, F>(self, f: F) -> Edomaton<Env, R, E, N, B>
    where
        B: Send + 'static,
        F: Fn(A) -> B + Send + Sync + 'static,
    {
        self.transform(move |r| r.map(&f))
    }

    /// Replaces the output.
    pub fn replace<B>(self, b: B) -> Edomaton<Env, R, E, N, B>
    where
        B: Clone + Send + Sync + 'static,
    {
        self.map(move |_| b.clone())
    }

    /// Ignores the output.
    pub fn void(self) -> Edomaton<Env, R, E, N, ()> {
        self.map(|_| ())
    }

    /// Creates a new edomaton that translates some input to what this one
    /// can understand.
    pub fn contramap<Env2, F>(self, f: F) -> Edomaton<Env2, R, E, N, A>
    where
        Env2: Send + 'static,
        F: Fn(Env2) -> Env + Send + Sync + 'static,
    {
        Edomaton::new(move |env2| self.run(f(env2)))
    }

    /// Binds another edomaton to this one.
    ///
    /// Events and notifications accumulate on success. If this edomaton is
    /// rejected, `f` is not called. If the next edomaton is rejected, only
    /// its notifications are kept.
    pub fn and_then<B, F>(self, f: F) -> Edomaton<Env, R, E, N, B>
    where
        Env: Clone,
        B: Send + 'static,
        F: Fn(A) -> Edomaton<Env, R, E, N, B> + Send + Sync + 'static,
    {
        let f = Arc::new(f);
        Edomaton::new(move |env: Env| {
            let first = self.run(env.clone());
            let f = Arc::clone(&f);
            async move {
                let r = first.await;
                match r.result {
                    Decision::Rejected(reasons) => {
                        ResponseD::new(Decision::Rejected(reasons), r.notifications)
                    }
                    Decision::InDecisive(value) => {
                        let out = f(value).run(env).await;
                        ResponseD::new(Decision::InDecisive(()), r.notifications).then(out)
                    }
                    Decision::Accepted { events, result } => {
                        let out = f(result).run(env).await;
                        ResponseD::new(Decision::Accepted { events, result: () }, r.notifications)
                            .then(out)
                    }
                }
            }
        })
    }

    /// Alias of [`Edomaton::and_then`].
    pub fn flat_map<B, F>(self, f: F) -> Edomaton<Env, R, E, N, B>
    where
        Env: Clone,
        B: Send + 'static,
        F: Fn(A) -> Edomaton<Env, R, E, N, B> + Send + Sync + 'static,
    {
        self.and_then(f)
    }

    /// Sequences another edomaton after this one, ignoring this output.
    pub fn then<B>(self, next: Edomaton<Env, R, E, N, B>) -> Edomaton<Env, R, E, N, B>
    where
        Env: Clone,
        B: Send + 'static,
    {
        self.and_then(move |_| next.clone())
    }

    /// Evaluates an effect using the output and uses its result as the new
    /// output.
    pub fn eval_map<B, F, Fut>(self, f: F) -> Edomaton<Env, R, E, N, B>
    where
        B: Send + 'static,
        F: Fn(A) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = B> + Send + 'static,
    {
        let f = Arc::new(f);
        Edomaton::new(move |env| {
            let first = self.run(env);
            let f = Arc::clone(&f);
            async move {
                let r = first.await;
                let result = match r.result {
                    Decision::Rejected(reasons) => Decision::Rejected(reasons),
                    Decision::InDecisive(a) => Decision::InDecisive(f(a).await),
                    Decision::Accepted { events, result } => Decision::Accepted {
                        events,
                        result: f(result).await,
                    },
                };
                ResponseD::new(result, r.notifications)
            }
        })
    }

    /// Evaluates an effect using the output, keeping the output unchanged.
    pub fn eval_tap<B, F, Fut>(self, f: F) -> Edomaton<Env, R, E, N, A>
    where
        B: Send + 'static,
        F: Fn(&A) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = B> + Send + 'static,
    {
        self.eval_map(move |a| {
            let fut = f(&a);
            async move {
                fut.await;
                a
            }
        })
    }

    /// Evaluates an effect after this edomaton, keeping the output.
    pub fn eval_with<B, F, Fut>(self, f: F) -> Edomaton<Env, R, E, N, A>
    where
        B: Send + 'static,
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = B> + Send + 'static,
    {
        self.eval_tap(move |_| f())
    }

    /// Decides based on the output.
    pub fn decide_with<B, F>(self, f: F) -> Edomaton<Env, R, E, N, B>
    where
        B: Send + 'static,
        F: Fn(A) -> Decision<R, E, B> + Send + Sync + 'static,
    {
        self.transform(move |r| r.and_then(|a| ResponseD::lift(f(a))))
    }

    /// Clears all notifications so far.
    pub fn reset(self) -> Self {
        self.transform(ResponseD::reset)
    }

    /// Adds notifications regardless of the decision state.
    pub fn publish<I>(self, ns: I) -> Self
    where
        I: IntoIterator<Item = N> + Clone + Send + Sync + 'static,
    {
        self.transform(move |r| r.publish(ns.clone()))
    }

    /// If this edomaton is rejected, uses `f` to decide what to publish.
    pub fn publish_on_rejection_with<F, I>(self, f: F) -> Self
    where
        F: Fn(&NonEmpty<R>) -> I + Send + Sync + 'static,
        I: IntoIterator<Item = N>,
    {
        self.transform(move |r| r.publish_on_rejection_with(&f))
    }

    /// Publishes the given notifications if this edomaton is rejected.
    pub fn publish_on_rejection<I>(self, ns: I) -> Self
    where
        I: IntoIterator<Item = N> + Clone + Send + Sync + 'static,
    {
        self.transform(move |r| r.publish_on_rejection(ns.clone()))
    }

    /// Recovers from a rejection.
    pub fn handle_error_with<F>(self, f: F) -> Self
    where
        Env: Clone,
        F: Fn(NonEmpty<R>) -> Edomaton<Env, R, E, N, A> + Send + Sync + 'static,
    {
        let f = Arc::new(f);
        Edomaton::new(move |env: Env| {
            let first = self.run(env.clone());
            let f = Arc::clone(&f);
            async move {
                let r = first.await;
                match r.result {
                    Decision::Rejected(reasons) => f(reasons).run(env).await,
                    other => ResponseD::new(other, r.notifications),
                }
            }
        })
    }
}

impl<Env, R, E, N> Edomaton<Env, R, E, N, ()>
where
    Env: Send + 'static,
    R: Send + 'static,
    E: Send + 'static,
    N: Send + 'static,
{
    /// An edomaton with a trivial output.
    pub fn unit() -> Self {
        Self::from_fn(|_| ResponseD::unit())
    }

    /// Constructs an edomaton that publishes the given notifications.
    pub fn publish_only<I>(ns: I) -> Self
    where
        I: IntoIterator<Item = N> + Clone + Send + Sync + 'static,
    {
        Self::from_fn(move |_| ResponseD::publish_only(ns.clone()))
    }

    /// Constructs an edomaton that accepts the given events.
    pub fn accept(events: impl Into<NonEmpty<E>>) -> Self
    where
        E: Clone + Sync,
    {
        let events = events.into();
        Self::from_fn(move |_| ResponseD::accept(events.clone()))
    }
}

impl<Env, R, E, N> Edomaton<Env, R, E, N, Env>
where
    Env: Send + 'static,
    R: Send + 'static,
    E: Send + 'static,
    N: Send + 'static,
{
    /// Constructs an edomaton that outputs its input.
    pub fn read() -> Self {
        Self::from_fn(ResponseD::pure)
    }
}
