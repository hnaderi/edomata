//! [`DecisionT`]: an asynchronous [`Decision`].

use std::fmt;
use std::future::Future;
use std::ops::ControlFlow;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::{BoxFuture, Decision, NonEmpty};

/// An effectful program that yields a [`Decision`].
///
/// `DecisionT` is a future: `.await` it to obtain the decision. Sequencing
/// with [`DecisionT::and_then`] has the same semantics as
/// [`Decision::and_then`]: events accumulate and rejections short-circuit.
///
/// ```
/// use edomata_core::{Decision, DecisionT, nonempty};
///
/// let program: DecisionT<&str, i32, ()> =
///     DecisionT::accept(1).and_then(|_| DecisionT::lift_future(async { 2 }).and_then_decision(Decision::accept));
/// let decision = futures::executor::block_on(program);
/// assert_eq!(decision, Decision::Accepted { events: nonempty![1, 2], result: () });
/// ```
pub struct DecisionT<R, E, A> {
    inner: BoxFuture<'static, Decision<R, E, A>>,
}

impl<R, E, A> fmt::Debug for DecisionT<R, E, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DecisionT(<future>)")
    }
}

impl<R, E, A> Future for DecisionT<R, E, A> {
    type Output = Decision<R, E, A>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().inner.as_mut().poll(cx)
    }
}

impl<R, E, A> DecisionT<R, E, A>
where
    R: Send + 'static,
    E: Send + 'static,
    A: Send + 'static,
{
    /// Wraps a future that yields a decision.
    pub fn new<Fut>(fut: Fut) -> Self
    where
        Fut: Future<Output = Decision<R, E, A>> + Send + 'static,
    {
        Self {
            inner: Box::pin(fut),
        }
    }

    /// Lifts a decision.
    pub fn lift(decision: Decision<R, E, A>) -> Self {
        Self::new(std::future::ready(decision))
    }

    /// Lifts an effect, whose output becomes the program output.
    pub fn lift_future<Fut>(fut: Fut) -> Self
    where
        Fut: Future<Output = A> + Send + 'static,
    {
        Self::new(async move { Decision::pure(fut.await) })
    }

    /// Constructs a program that outputs a pure value.
    pub fn pure(a: A) -> Self {
        Self::lift(Decision::pure(a))
    }

    /// Constructs a program that uses a validation to decide whether to
    /// output a value or reject.
    pub fn validate(validation: Result<A, NonEmpty<R>>) -> Self {
        Self::lift(Decision::validate(validation))
    }

    /// Constructs a program that decides to accept a sequence of events and
    /// returns an output.
    pub fn accept_return(result: A, events: impl Into<NonEmpty<E>>) -> Self {
        Self::lift(Decision::accept_return(result, events))
    }

    /// Constructs a program that decides to reject with a sequence of
    /// reasons.
    pub fn reject(reasons: impl Into<NonEmpty<R>>) -> Self {
        Self::lift(Decision::reject(reasons))
    }

    /// Runs this program.
    pub fn run(self) -> BoxFuture<'static, Decision<R, E, A>> {
        self.inner
    }

    /// Changes the output.
    pub fn map<B, F>(self, f: F) -> DecisionT<R, E, B>
    where
        B: Send + 'static,
        F: FnOnce(A) -> B + Send + 'static,
    {
        DecisionT::new(async move { self.await.map(f) })
    }

    /// Replaces the output.
    pub fn replace<B: Send + 'static>(self, b: B) -> DecisionT<R, E, B> {
        self.map(|_| b)
    }

    /// Binds another program to this one.
    pub fn and_then<B, F>(self, f: F) -> DecisionT<R, E, B>
    where
        B: Send + 'static,
        F: FnOnce(A) -> DecisionT<R, E, B> + Send + 'static,
    {
        DecisionT::new(async move {
            match self.await {
                Decision::Accepted { events, result } => match f(result).await {
                    Decision::Accepted {
                        events: events2,
                        result,
                    } => Decision::Accepted {
                        events: events.concat(events2),
                        result,
                    },
                    Decision::InDecisive(result) => Decision::Accepted { events, result },
                    Decision::Rejected(reasons) => Decision::Rejected(reasons),
                },
                Decision::InDecisive(result) => f(result).await,
                Decision::Rejected(reasons) => Decision::Rejected(reasons),
            }
        })
    }

    /// Alias of [`DecisionT::and_then`].
    pub fn flat_map<B, F>(self, f: F) -> DecisionT<R, E, B>
    where
        B: Send + 'static,
        F: FnOnce(A) -> DecisionT<R, E, B> + Send + 'static,
    {
        self.and_then(f)
    }

    /// Binds a pure decision to this program.
    pub fn and_then_decision<B, F>(self, f: F) -> DecisionT<R, E, B>
    where
        B: Send + 'static,
        F: FnOnce(A) -> Decision<R, E, B> + Send + 'static,
    {
        DecisionT::new(async move { self.await.and_then(f) })
    }

    /// Recovers from a rejection.
    pub fn handle_error_with<F>(self, f: F) -> DecisionT<R, E, A>
    where
        F: FnOnce(NonEmpty<R>) -> DecisionT<R, E, A> + Send + 'static,
    {
        DecisionT::new(async move {
            match self.await {
                Decision::Rejected(reasons) => f(reasons).await,
                other => other,
            }
        })
    }

    /// Stack-safe iteration, the counterpart of Cats' `tailRecM`.
    pub fn tail_rec<S, F>(init: S, mut f: F) -> DecisionT<R, E, A>
    where
        S: Send + 'static,
        F: FnMut(S) -> DecisionT<R, E, ControlFlow<A, S>> + Send + 'static,
    {
        DecisionT::new(async move {
            let mut acc: Vec<E> = Vec::new();
            let mut state = init;
            loop {
                match f(state).await {
                    Decision::Accepted { events, result } => match result {
                        ControlFlow::Continue(s) => {
                            acc.extend(events);
                            state = s;
                        }
                        ControlFlow::Break(b) => {
                            let mut events = events;
                            events.prepend(acc);
                            return Decision::Accepted { events, result: b };
                        }
                    },
                    Decision::InDecisive(result) => match result {
                        ControlFlow::Continue(s) => state = s,
                        ControlFlow::Break(b) => {
                            return match NonEmpty::from_vec(acc) {
                                None => Decision::InDecisive(b),
                                Some(events) => Decision::Accepted { events, result: b },
                            };
                        }
                    },
                    Decision::Rejected(reasons) => return Decision::Rejected(reasons),
                }
            }
        })
    }
}

impl<R, E> DecisionT<R, E, ()>
where
    R: Send + 'static,
    E: Send + 'static,
{
    /// Constructs a program with a trivial output.
    pub fn unit() -> Self {
        Self::lift(Decision::unit())
    }

    /// Constructs a program that decides to accept a sequence of events.
    pub fn accept(events: impl Into<NonEmpty<E>>) -> Self {
        Self::lift(Decision::accept(events))
    }
}

impl<R, E, A> From<Decision<R, E, A>> for DecisionT<R, E, A>
where
    R: Send + 'static,
    E: Send + 'static,
    A: Send + 'static,
{
    fn from(decision: Decision<R, E, A>) -> Self {
        Self::lift(decision)
    }
}
