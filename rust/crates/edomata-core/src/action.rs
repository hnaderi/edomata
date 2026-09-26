//! [`Action`]: an asynchronous [`ResponseD`].

use std::fmt;
use std::future::Future;
use std::ops::ControlFlow;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::{BoxFuture, Decision, NonEmpty, ResponseD};

/// An effectful program that yields a [`ResponseD`]: a decision plus
/// notifications.
///
/// `Action` is a future: `.await` it to obtain the response.
///
/// ```
/// use edomata_core::{Action, Decision, nonempty};
///
/// let program: Action<&str, i32, &str, ()> =
///     Action::accept(1).publish(["one"]).and_then(|_| Action::accept(2).publish(["two"]));
/// let response = futures::executor::block_on(program);
/// assert_eq!(response.result, Decision::Accepted { events: nonempty![1, 2], result: () });
/// assert_eq!(response.notifications, vec!["one", "two"]);
/// ```
pub struct Action<R, E, N, A> {
    inner: BoxFuture<'static, ResponseD<R, E, N, A>>,
}

impl<R, E, N, A> fmt::Debug for Action<R, E, N, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Action(<future>)")
    }
}

impl<R, E, N, A> Future for Action<R, E, N, A> {
    type Output = ResponseD<R, E, N, A>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().inner.as_mut().poll(cx)
    }
}

impl<R, E, N, A> Action<R, E, N, A>
where
    R: Send + 'static,
    E: Send + 'static,
    N: Send + 'static,
    A: Send + 'static,
{
    /// Wraps a future that yields a response.
    pub fn new<Fut>(fut: Fut) -> Self
    where
        Fut: Future<Output = ResponseD<R, E, N, A>> + Send + 'static,
    {
        Self {
            inner: Box::pin(fut),
        }
    }

    /// Lifts a response.
    pub fn lift(response: ResponseD<R, E, N, A>) -> Self {
        Self::new(std::future::ready(response))
    }

    /// Lifts a decision.
    pub fn lift_decision(decision: Decision<R, E, A>) -> Self {
        Self::lift(ResponseD::lift(decision))
    }

    /// Lifts an effect, whose output becomes the program output.
    pub fn lift_future<Fut>(fut: Fut) -> Self
    where
        Fut: Future<Output = A> + Send + 'static,
    {
        Self::new(async move { ResponseD::pure(fut.await) })
    }

    /// Constructs a program that outputs a pure value.
    pub fn pure(a: A) -> Self {
        Self::lift(ResponseD::pure(a))
    }

    /// Constructs a program that uses a validation to decide whether to
    /// output a value or reject.
    pub fn validate(validation: Result<A, NonEmpty<R>>) -> Self {
        Self::lift_decision(Decision::validate(validation))
    }

    /// Constructs a program that decides to reject with a sequence of
    /// reasons.
    pub fn reject(reasons: impl Into<NonEmpty<R>>) -> Self {
        Self::lift_decision(Decision::reject(reasons))
    }

    /// Runs this program.
    pub fn run(self) -> BoxFuture<'static, ResponseD<R, E, N, A>> {
        self.inner
    }

    /// Changes the output.
    pub fn map<B, F>(self, f: F) -> Action<R, E, N, B>
    where
        B: Send + 'static,
        F: FnOnce(A) -> B + Send + 'static,
    {
        Action::new(async move { self.await.map(f) })
    }

    /// Replaces the output.
    pub fn replace<B: Send + 'static>(self, b: B) -> Action<R, E, N, B> {
        self.map(|_| b)
    }

    /// Binds another program to this one.
    ///
    /// Events and notifications accumulate on success; a rejection
    /// terminates the chain and only keeps the notifications of the
    /// rejecting step.
    pub fn and_then<B, F>(self, f: F) -> Action<R, E, N, B>
    where
        B: Send + 'static,
        F: FnOnce(A) -> Action<R, E, N, B> + Send + 'static,
    {
        Action::new(async move {
            let a = self.await;
            match a.result {
                Decision::Rejected(reasons) => {
                    ResponseD::new(Decision::Rejected(reasons), a.notifications)
                }
                Decision::InDecisive(value) => {
                    let out = f(value).await;
                    ResponseD::new(Decision::InDecisive(()), a.notifications).then(out)
                }
                Decision::Accepted { events, result } => {
                    let out = f(result).await;
                    ResponseD::new(Decision::Accepted { events, result: () }, a.notifications)
                        .then(out)
                }
            }
        })
    }

    /// Alias of [`Action::and_then`].
    pub fn flat_map<B, F>(self, f: F) -> Action<R, E, N, B>
    where
        B: Send + 'static,
        F: FnOnce(A) -> Action<R, E, N, B> + Send + 'static,
    {
        self.and_then(f)
    }

    /// Sequences `next` after this program, ignoring this output.
    pub fn then<B: Send + 'static>(self, next: Action<R, E, N, B>) -> Action<R, E, N, B> {
        self.and_then(|_| next)
    }

    /// Clears all notifications so far.
    pub fn reset(self) -> Self {
        Action::new(async move { self.await.reset() })
    }

    /// Adds notifications regardless of the decision state.
    pub fn publish<I>(self, ns: I) -> Self
    where
        I: IntoIterator<Item = N> + Send + 'static,
    {
        Action::new(async move { self.await.publish(ns) })
    }

    /// Stack-safe iteration, the counterpart of Cats' `tailRecM`.
    pub fn tail_rec<S, F>(init: S, mut f: F) -> Action<R, E, N, A>
    where
        S: Send + 'static,
        F: FnMut(S) -> Action<R, E, N, ControlFlow<A, S>> + Send + 'static,
    {
        Action::new(async move {
            let mut ns0: Vec<N> = Vec::new();
            let mut evs0: Vec<E> = Vec::new();
            let mut state = init;
            loop {
                let res = f(state).await;
                match res.result {
                    Decision::Accepted { events, result } => match result {
                        ControlFlow::Continue(s) => {
                            ns0.extend(res.notifications);
                            evs0.extend(events);
                            state = s;
                        }
                        ControlFlow::Break(b) => {
                            let mut events = events;
                            events.prepend(evs0);
                            ns0.extend(res.notifications);
                            return ResponseD::new(Decision::Accepted { events, result: b }, ns0);
                        }
                    },
                    Decision::InDecisive(result) => match result {
                        ControlFlow::Continue(s) => {
                            ns0.extend(res.notifications);
                            state = s;
                        }
                        ControlFlow::Break(b) => {
                            ns0.extend(res.notifications);
                            let decision = match NonEmpty::from_vec(evs0) {
                                None => Decision::InDecisive(b),
                                Some(events) => Decision::Accepted { events, result: b },
                            };
                            return ResponseD::new(decision, ns0);
                        }
                    },
                    Decision::Rejected(reasons) => {
                        return ResponseD::new(Decision::Rejected(reasons), res.notifications);
                    }
                }
            }
        })
    }
}

impl<R, E, N> Action<R, E, N, ()>
where
    R: Send + 'static,
    E: Send + 'static,
    N: Send + 'static,
{
    /// Constructs a program with a trivial output.
    pub fn void() -> Self {
        Self::pure(())
    }

    /// Alias of [`Action::void`].
    pub fn unit() -> Self {
        Self::pure(())
    }

    /// Constructs a program that decides to accept a sequence of events.
    pub fn accept(events: impl Into<NonEmpty<E>>) -> Self {
        Self::lift_decision(Decision::accept(events))
    }

    /// Constructs a program that publishes the given notifications.
    pub fn publish_only(ns: impl IntoIterator<Item = N>) -> Self {
        Self::lift(ResponseD::publish_only(ns))
    }
}
