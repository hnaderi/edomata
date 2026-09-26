//! [`Stomaton`]: the state automaton used for CQRS (no event sourcing).

use std::fmt;
use std::future::Future;
use std::sync::Arc;

use crate::{BoxFuture, NonEmpty, ResponseE};

/// The reusable program behind a [`Stomaton`].
type RunFn<Env, S, R, N, A> =
    dyn Fn(Env, S) -> BoxFuture<'static, ResponseE<R, N, (S, A)>> + Send + Sync;

/// A state automaton: a reusable, asynchronous program that reads an input
/// (`Env`) and the current state, produces a new state and an output, and
/// optionally publishes notifications. Unlike [`Edomaton`](crate::Edomaton)
/// it has no domain events: the state is stored directly.
///
/// # Type parameters
///
/// * `Env` – input type, usually a [`CommandMessage`](crate::CommandMessage)
/// * `S` – state type
/// * `R` – rejection type
/// * `N` – notification type
/// * `A` – output type
///
/// ```
/// use edomata_core::{Stomaton, nonempty};
///
/// let inc: Stomaton<i32, i32, &str, &str, i32> = Stomaton::context()
///     .and_then(|by| Stomaton::modify(move |s| s + by))
///     .publish(["incremented"]);
///
/// let out = futures::executor::block_on(inc.run(5, 10));
/// assert_eq!(out.result, Ok((15, 15)));
/// assert_eq!(out.notifications, vec!["incremented"]);
/// ```
pub struct Stomaton<Env, S, R, N, A> {
    run: Arc<RunFn<Env, S, R, N, A>>,
}

impl<Env, S, R, N, A> Clone for Stomaton<Env, S, R, N, A> {
    fn clone(&self) -> Self {
        Self {
            run: Arc::clone(&self.run),
        }
    }
}

impl<Env, S, R, N, A> fmt::Debug for Stomaton<Env, S, R, N, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Stomaton(<program>)")
    }
}

impl<Env, S, R, N, A> Stomaton<Env, S, R, N, A>
where
    Env: Send + 'static,
    S: Send + 'static,
    R: Send + 'static,
    N: Send + 'static,
    A: Send + 'static,
{
    // ---------------------------------------------------------------------
    // Constructors
    // ---------------------------------------------------------------------

    /// Constructs a stomaton from an asynchronous function of its input and
    /// state.
    pub fn new<F, Fut>(f: F) -> Self
    where
        F: Fn(Env, S) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ResponseE<R, N, (S, A)>> + Send + 'static,
    {
        Self {
            run: Arc::new(move |env, s| Box::pin(f(env, s))),
        }
    }

    /// Constructs a stomaton from a pure function of its input and state.
    pub fn from_fn<F>(f: F) -> Self
    where
        F: Fn(Env, S) -> ResponseE<R, N, (S, A)> + Send + Sync + 'static,
    {
        Self::new(move |env, s| std::future::ready(f(env, s)))
    }

    /// Constructs a stomaton that outputs a pure value.
    pub fn pure(a: A) -> Self
    where
        A: Clone + Sync,
    {
        Self::from_fn(move |_, s| ResponseE::pure((s, a.clone())))
    }

    /// Constructs a stomaton that evaluates an effect, whose output becomes
    /// the program output.
    pub fn eval<F, Fut>(f: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = A> + Send + 'static,
    {
        Self::new(move |_, s| {
            let fut = f();
            async move { ResponseE::pure((s, fut.await)) }
        })
    }

    /// Constructs a stomaton that runs an effect using its input.
    pub fn run_with<F, Fut>(f: F) -> Self
    where
        F: Fn(Env) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = A> + Send + 'static,
    {
        Self::new(move |env, s| {
            let fut = f(env);
            async move { ResponseE::pure((s, fut.await)) }
        })
    }

    /// Constructs a stomaton that outputs a pure function of its input.
    pub fn map_input<F>(f: F) -> Self
    where
        F: Fn(Env) -> A + Send + Sync + 'static,
    {
        Self::from_fn(move |env, s| ResponseE::pure((s, f(env))))
    }

    /// Constructs a stomaton that outputs a value or rejects, without
    /// touching the state.
    pub fn decide(result: Result<A, NonEmpty<R>>) -> Self
    where
        Result<A, NonEmpty<R>>: Clone + Sync,
    {
        Self::from_fn(move |_, s| ResponseE::lift(result.clone().map(|a| (s, a))))
    }

    /// Constructs a stomaton that decides with a function of the current
    /// state, without touching the state.
    pub fn decide_with<F>(f: F) -> Self
    where
        F: Fn(&S) -> Result<A, NonEmpty<R>> + Send + Sync + 'static,
    {
        Self::from_fn(move |_, s| {
            let out = f(&s);
            ResponseE::lift(out.map(|a| (s, a)))
        })
    }

    /// Constructs a stomaton that rejects with the given reasons.
    pub fn reject(reasons: impl Into<NonEmpty<R>>) -> Self
    where
        R: Clone + Sync,
    {
        let reasons = reasons.into();
        Self::from_fn(move |_, _| ResponseE::reject(reasons.clone()))
    }

    /// Constructs a stomaton from a validation.
    pub fn validate(validation: Result<A, NonEmpty<R>>) -> Self
    where
        Result<A, NonEmpty<R>>: Clone + Sync,
    {
        Self::decide(validation)
    }

    /// Constructs a stomaton from an optional value, that outputs the value
    /// if it exists or rejects otherwise.
    pub fn from_option(opt: Option<A>, or_else: impl Into<NonEmpty<R>>) -> Self
    where
        Result<A, NonEmpty<R>>: Clone + Sync,
    {
        Self::decide(opt.ok_or_else(|| or_else.into()))
    }

    /// Constructs a stomaton that either outputs a value or rejects with a
    /// single reason.
    pub fn from_result(result: Result<A, R>) -> Self
    where
        Result<A, NonEmpty<R>>: Clone + Sync,
    {
        Self::decide(result.map_err(NonEmpty::new))
    }

    /// Constructs a stomaton that either outputs a value or rejects with
    /// several reasons.
    pub fn from_result_nec(result: Result<A, NonEmpty<R>>) -> Self
    where
        Result<A, NonEmpty<R>>: Clone + Sync,
    {
        Self::decide(result)
    }

    // ---------------------------------------------------------------------
    // Running
    // ---------------------------------------------------------------------

    /// Runs this stomaton with the given input and state.
    pub fn run(&self, env: Env, state: S) -> BoxFuture<'static, ResponseE<R, N, (S, A)>> {
        (self.run)(env, state)
    }

    // ---------------------------------------------------------------------
    // Combinators
    // ---------------------------------------------------------------------

    /// Transforms the underlying response.
    pub fn transform<R2, N2, B, F>(self, f: F) -> Stomaton<Env, S, R2, N2, B>
    where
        R2: Send + 'static,
        N2: Send + 'static,
        B: Send + 'static,
        F: Fn(ResponseE<R, N, (S, A)>) -> ResponseE<R2, N2, (S, B)> + Send + Sync + 'static,
    {
        let f = Arc::new(f);
        Stomaton::new(move |env, s| {
            let fut = self.run(env, s);
            let f = Arc::clone(&f);
            async move { f(fut.await) }
        })
    }

    /// Maps the output.
    pub fn map<B, F>(self, f: F) -> Stomaton<Env, S, R, N, B>
    where
        B: Send + 'static,
        F: Fn(A) -> B + Send + Sync + 'static,
    {
        self.transform(move |r| r.map(|(s, a)| (s, f(a))))
    }

    /// Replaces the output.
    pub fn replace<B>(self, b: B) -> Stomaton<Env, S, R, N, B>
    where
        B: Clone + Send + Sync + 'static,
    {
        self.map(move |_| b.clone())
    }

    /// Ignores the output.
    pub fn void(self) -> Stomaton<Env, S, R, N, ()> {
        self.map(|_| ())
    }

    /// Creates a new stomaton that translates some input to what this one
    /// can understand.
    pub fn contramap<Env2, F>(self, f: F) -> Stomaton<Env2, S, R, N, A>
    where
        Env2: Send + 'static,
        F: Fn(Env2) -> Env + Send + Sync + 'static,
    {
        Stomaton::new(move |env2, s| self.run(f(env2), s))
    }

    /// Binds another stomaton to this one.
    ///
    /// The next stomaton runs with the state produced by this one.
    /// Notifications of both are kept in order; a rejection of this stomaton
    /// short-circuits and keeps its notifications.
    pub fn and_then<B, F>(self, f: F) -> Stomaton<Env, S, R, N, B>
    where
        Env: Clone,
        B: Send + 'static,
        F: Fn(A) -> Stomaton<Env, S, R, N, B> + Send + Sync + 'static,
    {
        let f = Arc::new(f);
        Stomaton::new(move |env: Env, s| {
            let first = self.run(env.clone(), s);
            let f = Arc::clone(&f);
            async move {
                let out = first.await;
                match out.result {
                    Ok((new_state, a)) => {
                        let o = f(a).run(env, new_state).await;
                        let mut notifications = out.notifications;
                        notifications.extend(o.notifications);
                        ResponseE::new(o.result, notifications)
                    }
                    Err(errs) => ResponseE::new(Err(errs), out.notifications),
                }
            }
        })
    }

    /// Alias of [`Stomaton::and_then`].
    pub fn flat_map<B, F>(self, f: F) -> Stomaton<Env, S, R, N, B>
    where
        Env: Clone,
        B: Send + 'static,
        F: Fn(A) -> Stomaton<Env, S, R, N, B> + Send + Sync + 'static,
    {
        self.and_then(f)
    }

    /// Sequences another stomaton after this one, ignoring this output.
    pub fn then<B>(self, next: Stomaton<Env, S, R, N, B>) -> Stomaton<Env, S, R, N, B>
    where
        Env: Clone,
        B: Send + 'static,
    {
        self.and_then(move |_| next.clone())
    }

    /// Modifies the resulting state.
    pub fn modify_state<F>(self, f: F) -> Self
    where
        F: Fn(S) -> S + Send + Sync + 'static,
    {
        self.transform(move |r| r.map(|(s, a)| (f(s), a)))
    }

    /// Decides on a new state from the resulting state; the output becomes
    /// the new state.
    pub fn decide_state<F>(self, f: F) -> Stomaton<Env, S, R, N, S>
    where
        S: Clone,
        F: Fn(S) -> Result<S, NonEmpty<R>> + Send + Sync + 'static,
    {
        self.transform(move |r| {
            r.and_then(|(s, _)| ResponseE::lift(f(s).map(|ns| (ns.clone(), ns))))
        })
    }

    /// Decides on the output.
    pub fn decide_output<B, F>(self, f: F) -> Stomaton<Env, S, R, N, B>
    where
        B: Send + 'static,
        F: Fn(A) -> Result<B, NonEmpty<R>> + Send + Sync + 'static,
    {
        self.transform(move |r| r.and_then(|(s, a)| ResponseE::lift(f(a).map(|b| (s, b)))))
    }

    /// Sets the resulting state.
    pub fn set_state(self, state: S) -> Self
    where
        S: Clone + Sync,
    {
        self.transform(move |r| r.map(|(_, a)| (state.clone(), a)))
    }

    /// Recovers from a rejection, running `f` with the original state.
    pub fn handle_error_with<F>(self, f: F) -> Self
    where
        Env: Clone,
        S: Clone,
        F: Fn(NonEmpty<R>) -> Stomaton<Env, S, R, N, A> + Send + Sync + 'static,
    {
        let f = Arc::new(f);
        Stomaton::new(move |env: Env, s: S| {
            let first = self.run(env.clone(), s.clone());
            let f = Arc::clone(&f);
            async move {
                let out = first.await;
                match out.result {
                    Err(errs) => f(errs).run(env, s).await,
                    Ok(v) => ResponseE::new(Ok(v), out.notifications),
                }
            }
        })
    }

    /// Adds notifications regardless of the decision state.
    pub fn publish<I>(self, ns: I) -> Self
    where
        I: IntoIterator<Item = N> + Clone + Send + Sync + 'static,
    {
        self.transform(move |r| r.publish(ns.clone()))
    }

    /// If this stomaton is rejected, uses `f` to decide what to publish.
    pub fn publish_on_rejection_with<F, I>(self, f: F) -> Self
    where
        F: Fn(&NonEmpty<R>) -> I + Send + Sync + 'static,
        I: IntoIterator<Item = N>,
    {
        self.transform(move |r| r.publish_on_rejection_with(&f))
    }

    /// Publishes the given notifications if this stomaton is rejected.
    pub fn publish_on_rejection<I>(self, ns: I) -> Self
    where
        I: IntoIterator<Item = N> + Clone + Send + Sync + 'static,
    {
        self.transform(move |r| r.publish_on_rejection(ns.clone()))
    }
}

impl<Env, S, R, N> Stomaton<Env, S, R, N, ()>
where
    Env: Send + 'static,
    S: Send + 'static,
    R: Send + 'static,
    N: Send + 'static,
{
    /// A stomaton with a trivial output.
    pub fn unit() -> Self {
        Self::from_fn(|_, s| ResponseE::pure((s, ())))
    }

    /// Constructs a stomaton that sets the current state.
    pub fn set(state: S) -> Self
    where
        S: Clone + Sync,
    {
        Self::from_fn(move |_, _| ResponseE::pure((state.clone(), ())))
    }

    /// Constructs a stomaton that publishes the given notifications.
    pub fn publish_only<I>(ns: I) -> Self
    where
        I: IntoIterator<Item = N> + Clone + Send + Sync + 'static,
    {
        Self::from_fn(move |_, s| ResponseE::new(Ok((s, ())), ns.clone()))
    }
}

impl<Env, S, R, N> Stomaton<Env, S, R, N, S>
where
    Env: Send + 'static,
    S: Clone + Send + 'static,
    R: Send + 'static,
    N: Send + 'static,
{
    /// Constructs a stomaton that outputs the current state.
    pub fn state() -> Self {
        Self::from_fn(|_, s: S| ResponseE::pure((s.clone(), s)))
    }

    /// Constructs a stomaton that modifies the current state and outputs the
    /// new state.
    pub fn modify<F>(f: F) -> Self
    where
        F: Fn(S) -> S + Send + Sync + 'static,
    {
        Self::from_fn(move |_, s| {
            let ns = f(s);
            ResponseE::pure((ns.clone(), ns))
        })
    }

    /// Constructs a stomaton that decides to modify the state based on the
    /// current state and outputs the new state.
    pub fn modify_s<F>(f: F) -> Self
    where
        F: Fn(S) -> Result<S, NonEmpty<R>> + Send + Sync + 'static,
    {
        Self::from_fn(move |_, s| ResponseE::lift(f(s).map(|ns| (ns.clone(), ns))))
    }

    /// Alias of [`Stomaton::modify_s`].
    pub fn decide_s<F>(f: F) -> Self
    where
        F: Fn(S) -> Result<S, NonEmpty<R>> + Send + Sync + 'static,
    {
        Self::modify_s(f)
    }
}

impl<Env, S, R, N> Stomaton<Env, S, R, N, Env>
where
    Env: Send + 'static,
    S: Send + 'static,
    R: Send + 'static,
    N: Send + 'static,
{
    /// Constructs a stomaton that outputs its input.
    pub fn context() -> Self {
        Self::from_fn(|env, s| ResponseE::pure((s, env)))
    }
}
