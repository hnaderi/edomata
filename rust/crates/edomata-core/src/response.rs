//! [`ResponseT`]: a result combined with notifications.
//!
//! [`ResponseD`] pairs a [`Decision`] with notifications and [`ResponseE`]
//! pairs a `Result` with notifications. Both are aliases of [`ResponseT`],
//! which is generic over any result type implementing [`RaiseError`].

use std::ops::ControlFlow;

use crate::{Decision, NonEmpty};

/// Abstraction over result-like types that can carry rejections.
///
/// This is the Rust counterpart of Scala's `RaiseError[F, R]` typeclass
/// combined with the `MonadError` instance that `ResponseT` needs. It is
/// implemented for [`Decision`] and for `Result<A, NonEmpty<R>>`.
pub trait RaiseError: Sized {
    /// Rejection type.
    type Rejection;
    /// Output type.
    type Output;
    /// The same result type with a different output.
    type WithOutput<B>: RaiseError<Rejection = Self::Rejection, Output = B>;

    /// Builds a rejected result.
    fn raise(errs: NonEmpty<Self::Rejection>) -> Self;
    /// Builds a successful result.
    fn pure(value: Self::Output) -> Self;
    /// Borrows the output or the rejections.
    fn as_result(&self) -> Result<&Self::Output, &NonEmpty<Self::Rejection>>;
    /// Whether this result is a rejection.
    fn is_error(&self) -> bool {
        self.as_result().is_err()
    }
    /// Consumes this result into the output or the rejections.
    fn into_result(self) -> Result<Self::Output, NonEmpty<Self::Rejection>>;
    /// Changes the output.
    fn map<B, F: FnOnce(Self::Output) -> B>(self, f: F) -> Self::WithOutput<B>;
    /// Sequences another result after this one.
    fn and_then<B, F: FnOnce(Self::Output) -> Self::WithOutput<B>>(
        self,
        f: F,
    ) -> Self::WithOutput<B>;
    /// Folds this result.
    fn fold<B, FE, FA>(self, on_error: FE, on_value: FA) -> B
    where
        FE: FnOnce(NonEmpty<Self::Rejection>) -> B,
        FA: FnOnce(Self::Output) -> B,
    {
        match self.into_result() {
            Ok(a) => on_value(a),
            Err(e) => on_error(e),
        }
    }
    /// Separates the output of `value` from its carrier (for a [`Decision`],
    /// the accepted events), or returns the rejections.
    #[allow(clippy::type_complexity)]
    fn split<B>(
        value: Self::WithOutput<B>,
    ) -> Result<(Self::WithOutput<()>, B), NonEmpty<Self::Rejection>>;
    /// Sequences `next` after a carrier produced by [`RaiseError::split`].
    fn join<B>(carrier: Self::WithOutput<()>, next: Self::WithOutput<B>) -> Self::WithOutput<B>;
}

impl<R, E, A> RaiseError for Decision<R, E, A> {
    type Rejection = R;
    type Output = A;
    type WithOutput<B> = Decision<R, E, B>;

    fn raise(errs: NonEmpty<R>) -> Self {
        Decision::Rejected(errs)
    }

    fn pure(value: A) -> Self {
        Decision::InDecisive(value)
    }

    fn as_result(&self) -> Result<&A, &NonEmpty<R>> {
        Decision::as_result(self)
    }

    fn is_error(&self) -> bool {
        self.is_rejected()
    }

    fn into_result(self) -> Result<A, NonEmpty<R>> {
        self.to_result()
    }

    fn map<B, F: FnOnce(A) -> B>(self, f: F) -> Decision<R, E, B> {
        Decision::map(self, f)
    }

    fn and_then<B, F: FnOnce(A) -> Decision<R, E, B>>(self, f: F) -> Decision<R, E, B> {
        Decision::and_then(self, f)
    }

    fn split<B>(value: Decision<R, E, B>) -> Result<(Decision<R, E, ()>, B), NonEmpty<R>> {
        match value {
            Decision::InDecisive(a) => Ok((Decision::InDecisive(()), a)),
            Decision::Accepted { events, result } => {
                Ok((Decision::Accepted { events, result: () }, result))
            }
            Decision::Rejected(reasons) => Err(reasons),
        }
    }

    fn join<B>(carrier: Decision<R, E, ()>, next: Decision<R, E, B>) -> Decision<R, E, B> {
        carrier.then(next)
    }
}

impl<R, A> RaiseError for Result<A, NonEmpty<R>> {
    type Rejection = R;
    type Output = A;
    type WithOutput<B> = Result<B, NonEmpty<R>>;

    fn raise(errs: NonEmpty<R>) -> Self {
        Err(errs)
    }

    fn pure(value: A) -> Self {
        Ok(value)
    }

    fn as_result(&self) -> Result<&A, &NonEmpty<R>> {
        self.as_ref()
    }

    fn into_result(self) -> Result<A, NonEmpty<R>> {
        self
    }

    fn map<B, F: FnOnce(A) -> B>(self, f: F) -> Result<B, NonEmpty<R>> {
        Result::map(self, f)
    }

    fn and_then<B, F: FnOnce(A) -> Result<B, NonEmpty<R>>>(self, f: F) -> Result<B, NonEmpty<R>> {
        Result::and_then(self, f)
    }

    fn split<B>(
        value: Result<B, NonEmpty<R>>,
    ) -> Result<(Result<(), NonEmpty<R>>, B), NonEmpty<R>> {
        value.map(|b| (Ok(()), b))
    }

    fn join<B>(
        carrier: Result<(), NonEmpty<R>>,
        next: Result<B, NonEmpty<R>>,
    ) -> Result<B, NonEmpty<R>> {
        carrier.and(next)
    }
}

/// A result of type `Res` together with the notifications published so far.
///
/// Notifications accumulate on success and are reset when the sequenced
/// program is rejected: only the notifications of the rejecting step are
/// kept. A rejected response never changes when further programs are
/// sequenced after it.
///
/// ```
/// use edomata_core::{Decision, ResponseD, ResponseT};
///
/// let a: ResponseD<&str, i32, &str, ()> = ResponseD::accept(1).publish(["created"]);
/// let b: ResponseD<&str, i32, &str, ()> = ResponseT::reject("nope").publish(["failed"]);
/// let c = a.clone().then(b);
/// assert!(c.result.is_rejected());
/// assert_eq!(c.notifications, vec!["failed"]);
/// assert_eq!(a.notifications, vec!["created"]);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ResponseT<Res, N> {
    /// The underlying result.
    pub result: Res,
    /// Notifications published so far, in order.
    pub notifications: Vec<N>,
}

/// A [`Decision`] together with notifications.
pub type ResponseD<R, E, N, A> = ResponseT<Decision<R, E, A>, N>;

/// A `Result<A, NonEmpty<R>>` together with notifications.
pub type ResponseE<R, N, A> = ResponseT<Result<A, NonEmpty<R>>, N>;

impl<Res, N> ResponseT<Res, N> {
    /// Constructs a response from a result and notifications.
    pub fn new(result: Res, notifications: impl IntoIterator<Item = N>) -> Self {
        Self {
            result,
            notifications: notifications.into_iter().collect(),
        }
    }

    /// Constructs a response from a result, without notifications.
    pub fn lift(result: Res) -> Self {
        Self {
            result,
            notifications: Vec::new(),
        }
    }

    /// Clears all notifications so far.
    pub fn reset(mut self) -> Self {
        self.notifications.clear();
        self
    }

    /// Adds notifications regardless of the result.
    pub fn publish(mut self, ns: impl IntoIterator<Item = N>) -> Self {
        self.notifications.extend(ns);
        self
    }

    /// Changes the notification type.
    pub fn map_notifications<N2, F: FnMut(N) -> N2>(self, f: F) -> ResponseT<Res, N2> {
        ResponseT {
            result: self.result,
            notifications: self.notifications.into_iter().map(f).collect(),
        }
    }
}

impl<Res: RaiseError, N> ResponseT<Res, N> {
    /// Constructs a response that outputs a pure value.
    pub fn pure(value: Res::Output) -> Self {
        Self::lift(Res::pure(value))
    }

    /// Constructs a response that publishes the given notifications.
    pub fn publish_only(ns: impl IntoIterator<Item = N>) -> Self
    where
        Res: RaiseError<Output = ()>,
    {
        Self::new(Res::pure(()), ns)
    }

    /// Constructs a response that rejects with the given reasons.
    pub fn reject(reasons: impl Into<NonEmpty<Res::Rejection>>) -> Self {
        Self::lift(Res::raise(reasons.into()))
    }

    /// Constructs a response from a validation.
    pub fn validate(validation: Result<Res::Output, NonEmpty<Res::Rejection>>) -> Self {
        match validation {
            Ok(a) => Self::pure(a),
            Err(e) => Self::lift(Res::raise(e)),
        }
    }

    /// Whether the underlying result is a rejection.
    pub fn is_rejected(&self) -> bool {
        self.result.is_error()
    }

    /// Changes the output.
    pub fn map<B, F: FnOnce(Res::Output) -> B>(self, f: F) -> ResponseT<Res::WithOutput<B>, N> {
        ResponseT {
            result: self.result.map(f),
            notifications: self.notifications,
        }
    }

    /// Replaces the output.
    pub fn replace<B>(self, b: B) -> ResponseT<Res::WithOutput<B>, N> {
        self.map(|_| b)
    }

    /// Sequences another response after this one.
    ///
    /// On success, notifications of both responses are kept in order. If the
    /// next response is rejected, only its notifications are kept. If this
    /// response is already rejected, `f` is never called.
    pub fn and_then<B, F>(self, f: F) -> ResponseT<Res::WithOutput<B>, N>
    where
        F: FnOnce(Res::Output) -> ResponseT<Res::WithOutput<B>, N>,
    {
        let mut next_notifications = None;
        let result = self.result.and_then(|a| {
            let out = f(a);
            next_notifications = Some(out.notifications);
            out.result
        });
        match next_notifications {
            None => ResponseT {
                result,
                notifications: self.notifications,
            },
            Some(ns) if result.is_error() => ResponseT {
                result,
                notifications: ns,
            },
            Some(ns) => {
                let mut notifications = self.notifications;
                notifications.extend(ns);
                ResponseT {
                    result,
                    notifications,
                }
            }
        }
    }

    /// Alias of [`ResponseT::and_then`].
    pub fn flat_map<B, F>(self, f: F) -> ResponseT<Res::WithOutput<B>, N>
    where
        F: FnOnce(Res::Output) -> ResponseT<Res::WithOutput<B>, N>,
    {
        self.and_then(f)
    }

    /// Sequences `next` after this response, ignoring this output.
    pub fn then<B>(
        self,
        next: ResponseT<Res::WithOutput<B>, N>,
    ) -> ResponseT<Res::WithOutput<B>, N> {
        self.and_then(|_| next)
    }

    /// If rejected, uses `f` to decide what to publish.
    pub fn publish_on_rejection_with<F, I>(self, f: F) -> Self
    where
        F: FnOnce(&NonEmpty<Res::Rejection>) -> I,
        I: IntoIterator<Item = N>,
    {
        match self.result.as_result() {
            Err(errs) => {
                let ns: Vec<N> = f(errs).into_iter().collect();
                self.publish(ns)
            }
            Ok(_) => self,
        }
    }

    /// Publishes the given notifications if this response is rejected.
    pub fn publish_on_rejection(self, ns: impl IntoIterator<Item = N>) -> Self {
        self.publish_on_rejection_with(|_| ns)
    }

    /// Recovers from a rejection.
    pub fn handle_error_with<F>(self, f: F) -> Self
    where
        F: FnOnce(NonEmpty<Res::Rejection>) -> Self,
    {
        match self.result.into_result() {
            Err(errs) => f(errs),
            Ok(a) => ResponseT {
                result: Res::pure(a),
                notifications: self.notifications,
            },
        }
    }

    /// Stack-safe iteration, the counterpart of Cats' `tailRecM`.
    ///
    /// Events and notifications accumulate across iterations with the same
    /// rules as [`ResponseT::and_then`].
    pub fn tail_rec<S, B, F>(init: S, mut f: F) -> ResponseT<Res::WithOutput<B>, N>
    where
        F: FnMut(S) -> ResponseT<Res::WithOutput<ControlFlow<B, S>>, N>,
        Res: RaiseError<Output = S>,
    {
        let mut value: ResponseT<Res::WithOutput<ControlFlow<B, S>>, N> = f(init);
        loop {
            match Res::split(value.result) {
                Err(errs) => {
                    return ResponseT {
                        result: <Res::WithOutput<B>>::raise(errs),
                        notifications: value.notifications,
                    };
                }
                Ok((carrier, ControlFlow::Break(b))) => {
                    return ResponseT {
                        result: Res::join(carrier, <Res::WithOutput<B>>::pure(b)),
                        notifications: value.notifications,
                    };
                }
                Ok((carrier, ControlFlow::Continue(s))) => {
                    let next = f(s);
                    let result = Res::join(carrier, next.result);
                    let notifications = if result.is_error() {
                        next.notifications
                    } else {
                        let mut ns = value.notifications;
                        ns.extend(next.notifications);
                        ns
                    };
                    value = ResponseT {
                        result,
                        notifications,
                    };
                }
            }
        }
    }
}

impl<R, E, N, A> ResponseD<R, E, N, A> {
    /// Constructs a response that decides to accept a sequence of events and
    /// returns an output.
    pub fn accept_return(result: A, events: impl Into<NonEmpty<E>>) -> Self {
        Self::lift(Decision::accept_return(result, events))
    }

    /// Constructs a response from a decision.
    pub fn decide(decision: Decision<R, E, A>) -> Self {
        Self::lift(decision)
    }
}

impl<R, E, N> ResponseD<R, E, N, ()> {
    /// A response with a trivial output.
    pub fn unit() -> Self {
        Self::lift(Decision::unit())
    }

    /// Constructs a response that decides to accept a sequence of events.
    pub fn accept(events: impl Into<NonEmpty<E>>) -> Self {
        Self::lift(Decision::accept(events))
    }
}

impl<R, N> ResponseE<R, N, ()> {
    /// A response with a trivial output.
    pub fn unit() -> Self {
        Self::lift(Ok(()))
    }
}

impl<Res, N> From<Res> for ResponseT<Res, N> {
    fn from(result: Res) -> Self {
        Self::lift(result)
    }
}
