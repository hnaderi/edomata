//! [`Decision`]: the pure state machine that drives every Edomata program.

use std::ops::ControlFlow;

use crate::NonEmpty;

/// Result of a domain decision.
///
/// A `Decision` is a small state machine:
///
/// ```text
/// [*] -> InDecisive
/// InDecisive -- event --> Accepted
/// InDecisive -- join --> InDecisive
/// InDecisive -- rejection --> Rejected (resets and terminates)
/// Accepted -- event --> Accepted (accumulates)
/// Accepted -- rejection --> Rejected (resets and terminates)
/// ```
///
/// It behaves like a monad with error handling: [`Decision::and_then`]
/// accumulates events in order and short-circuits on the first rejection.
///
/// # Type parameters
///
/// * `R` – rejection type
/// * `E` – event type
/// * `A` – program output type
///
/// ```
/// use edomata_core::{Decision, nonempty};
///
/// let d = Decision::accept(1).and_then(|_| Decision::accept(2));
/// assert_eq!(d, Decision::Accepted { events: nonempty![1, 2], result: () });
///
/// let r: Decision<&str, i32, ()> = d.and_then(|_| Decision::reject("no"));
/// assert!(r.is_rejected());
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Decision<R, E, A> {
    /// No decision was made yet; the program produced an output only.
    InDecisive(A),
    /// The program accepted one or more events.
    Accepted {
        /// Events accepted so far, in order.
        events: NonEmpty<E>,
        /// The program output.
        result: A,
    },
    /// The program rejected the request with one or more reasons.
    Rejected(NonEmpty<R>),
}

impl<R, E, A> Decision<R, E, A> {
    // ---------------------------------------------------------------------
    // Constructors
    // ---------------------------------------------------------------------

    /// Constructs a program that outputs a pure value.
    pub fn pure(a: A) -> Self {
        Decision::InDecisive(a)
    }

    /// Constructs a program that decides to accept a sequence of events and
    /// also returns an output.
    pub fn accept_return(result: A, events: impl Into<NonEmpty<E>>) -> Self {
        Decision::Accepted {
            events: events.into(),
            result,
        }
    }

    /// Constructs a program that decides to reject with a sequence of reasons.
    pub fn reject(reasons: impl Into<NonEmpty<R>>) -> Self {
        Decision::Rejected(reasons.into())
    }

    /// Constructs a program that either outputs a value or rejects with all
    /// the given reasons.
    pub fn validate(validation: Result<A, NonEmpty<R>>) -> Self {
        match validation {
            Ok(a) => Decision::InDecisive(a),
            Err(reasons) => Decision::Rejected(reasons),
        }
    }

    /// Alias of [`Decision::validate`].
    pub fn from_result_nec(validation: Result<A, NonEmpty<R>>) -> Self {
        Self::validate(validation)
    }

    /// Constructs a program that either outputs a value or rejects with a
    /// single reason.
    pub fn from_result(result: Result<A, R>) -> Self {
        match result {
            Ok(a) => Decision::InDecisive(a),
            Err(reason) => Decision::Rejected(NonEmpty::new(reason)),
        }
    }

    /// Constructs a program from an optional value, that outputs the value if
    /// it exists or rejects otherwise.
    pub fn from_option(opt: Option<A>, or_else: impl Into<NonEmpty<R>>) -> Self {
        match opt {
            Some(a) => Decision::InDecisive(a),
            None => Decision::Rejected(or_else.into()),
        }
    }

    // ---------------------------------------------------------------------
    // Combinators
    // ---------------------------------------------------------------------

    /// Creates a new decision that changes the output value of this one.
    pub fn map<B, F: FnOnce(A) -> B>(self, f: F) -> Decision<R, E, B> {
        match self {
            Decision::InDecisive(a) => Decision::InDecisive(f(a)),
            Decision::Accepted { events, result } => Decision::Accepted {
                events,
                result: f(result),
            },
            Decision::Rejected(reasons) => Decision::Rejected(reasons),
        }
    }

    /// Binds another decision to this one, creating a new decision.
    ///
    /// Events accumulate in order, and a rejection terminates the chain.
    pub fn and_then<B, F: FnOnce(A) -> Decision<R, E, B>>(self, f: F) -> Decision<R, E, B> {
        match self {
            Decision::Accepted { events, result } => match f(result) {
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
            Decision::InDecisive(result) => f(result),
            Decision::Rejected(reasons) => Decision::Rejected(reasons),
        }
    }

    /// Alias of [`Decision::and_then`].
    pub fn flat_map<B, F: FnOnce(A) -> Decision<R, E, B>>(self, f: F) -> Decision<R, E, B> {
        self.and_then(f)
    }

    /// Sequences `next` after this decision, ignoring this output.
    pub fn then<B>(self, next: Decision<R, E, B>) -> Decision<R, E, B> {
        self.and_then(|_| next)
    }

    /// Replaces the output value.
    pub fn replace<B>(self, b: B) -> Decision<R, E, B> {
        self.map(|_| b)
    }

    /// Runs `f` for its events and rejections only, keeping this output.
    pub fn flat_tap<B, F: FnOnce(&A) -> Decision<R, E, B>>(self, f: F) -> Decision<R, E, A> {
        self.and_then(|a| f(&a).replace(a))
    }

    /// Whether this decision is rejected.
    pub fn is_rejected(&self) -> bool {
        matches!(self, Decision::Rejected(_))
    }

    /// Whether this decision is accepted.
    pub fn is_accepted(&self) -> bool {
        matches!(self, Decision::Accepted { .. })
    }

    /// Whether this decision is indecisive.
    pub fn is_indecisive(&self) -> bool {
        matches!(self, Decision::InDecisive(_))
    }

    /// Folds this decision: runs `fr` if it is rejected and `fa` otherwise.
    pub fn visit<B, FR, FA>(self, fr: FR, fa: FA) -> B
    where
        FR: FnOnce(NonEmpty<R>) -> B,
        FA: FnOnce(A) -> B,
    {
        match self {
            Decision::InDecisive(a) => fa(a),
            Decision::Accepted { result, .. } => fa(result),
            Decision::Rejected(reasons) => fr(reasons),
        }
    }

    /// Ignores events and converts this decision to a `Result`.
    pub fn to_result(self) -> Result<A, NonEmpty<R>> {
        self.visit(Err, Ok)
    }

    /// Ignores events and converts this decision to a `Result`, by reference.
    pub fn as_result(&self) -> Result<&A, &NonEmpty<R>> {
        match self {
            Decision::InDecisive(a) => Ok(a),
            Decision::Accepted { result, .. } => Ok(result),
            Decision::Rejected(reasons) => Err(reasons),
        }
    }

    /// Ignores events and errors and returns the program output, if any.
    pub fn to_option(self) -> Option<A> {
        self.visit(|_| None, Some)
    }

    /// The output value, if this decision is not rejected.
    pub fn result(&self) -> Option<&A> {
        self.as_result().ok()
    }

    /// The accepted events, if any.
    pub fn events(&self) -> Option<&NonEmpty<E>> {
        match self {
            Decision::Accepted { events, .. } => Some(events),
            _ => None,
        }
    }

    /// The rejection reasons, if this decision is rejected.
    pub fn rejections(&self) -> Option<&NonEmpty<R>> {
        self.as_result().err()
    }

    /// Ignores the output value.
    pub fn void(self) -> Decision<R, E, ()> {
        self.map(|_| ())
    }

    /// Validates the output with a function that may reject with several
    /// reasons.
    pub fn validate_with<B, F>(self, f: F) -> Decision<R, E, B>
    where
        F: FnOnce(A) -> Result<B, NonEmpty<R>>,
    {
        self.and_then(|a| Decision::validate(f(a)))
    }

    /// Validates the output with a function that may reject with a single
    /// reason.
    pub fn validate_one<B, F>(self, f: F) -> Decision<R, E, B>
    where
        F: FnOnce(A) -> Result<B, R>,
    {
        self.and_then(|a| Decision::from_result(f(a)))
    }

    /// Asserts the output with a function that may reject with several
    /// reasons, keeping the output unchanged.
    pub fn assert_with<B, F>(self, f: F) -> Decision<R, E, A>
    where
        F: FnOnce(&A) -> Result<B, NonEmpty<R>>,
    {
        self.flat_tap(|a| Decision::validate(f(a)))
    }

    /// Asserts the output with a function that may reject with a single
    /// reason, keeping the output unchanged.
    pub fn assert_one<B, F>(self, f: F) -> Decision<R, E, A>
    where
        F: FnOnce(&A) -> Result<B, R>,
    {
        self.flat_tap(|a| Decision::from_result(f(a)))
    }

    /// Recovers from a rejection.
    pub fn handle_error_with<F>(self, f: F) -> Decision<R, E, A>
    where
        F: FnOnce(NonEmpty<R>) -> Decision<R, E, A>,
    {
        match self {
            Decision::Rejected(reasons) => f(reasons),
            other => other,
        }
    }

    /// Stack-safe iteration: repeatedly applies `f` while it returns
    /// `Continue`, accumulating events along the way.
    ///
    /// This is the counterpart of Cats' `tailRecM`.
    pub fn tail_rec<S, F>(init: S, mut f: F) -> Decision<R, E, A>
    where
        F: FnMut(S) -> Decision<R, E, ControlFlow<A, S>>,
    {
        let mut acc: Vec<E> = Vec::new();
        let mut state = init;
        loop {
            match f(state) {
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
    }

    /// Folds the output into `init` unless this decision is rejected.
    pub fn fold_left<B, F: FnOnce(B, &A) -> B>(&self, init: B, f: F) -> B {
        match self.as_result() {
            Ok(a) => f(init, a),
            Err(_) => init,
        }
    }

    /// Converts the events to another type.
    pub fn map_events<E2, F: FnMut(E) -> E2>(self, f: F) -> Decision<R, E2, A> {
        match self {
            Decision::InDecisive(a) => Decision::InDecisive(a),
            Decision::Accepted { events, result } => Decision::Accepted {
                events: events.map(f),
                result,
            },
            Decision::Rejected(reasons) => Decision::Rejected(reasons),
        }
    }

    /// Converts the rejection reasons to another type.
    pub fn map_rejections<R2, F: FnMut(R) -> R2>(self, f: F) -> Decision<R2, E, A> {
        match self {
            Decision::InDecisive(a) => Decision::InDecisive(a),
            Decision::Accepted { events, result } => Decision::Accepted { events, result },
            Decision::Rejected(reasons) => Decision::Rejected(reasons.map(f)),
        }
    }
}

impl<R, E> Decision<R, E, ()> {
    /// Constructs a program with a trivial output.
    pub fn unit() -> Self {
        Decision::InDecisive(())
    }

    /// Constructs a program that decides to accept a sequence of events.
    ///
    /// A single event or a [`NonEmpty`] of events can be passed.
    pub fn accept(events: impl Into<NonEmpty<E>>) -> Self {
        Decision::Accepted {
            events: events.into(),
            result: (),
        }
    }

    /// Constructs a program that accepts a sequence of events when
    /// `predicate` holds, or does nothing otherwise.
    pub fn accept_when(predicate: bool, events: impl Into<NonEmpty<E>>) -> Self {
        if predicate {
            Self::accept(events)
        } else {
            Self::unit()
        }
    }

    /// Constructs a program that accepts an optional event or does nothing.
    pub fn accept_some(event: Option<E>) -> Self {
        match event {
            Some(e) => Self::accept(e),
            None => Self::unit(),
        }
    }

    /// Constructs a program that accepts an optional event or rejects
    /// otherwise.
    pub fn accept_some_or(event: Option<E>, or_else: impl Into<NonEmpty<R>>) -> Self {
        match event {
            Some(e) => Self::accept(e),
            None => Self::reject(or_else),
        }
    }

    /// Constructs a program that accepts an event or rejects with a single
    /// reason.
    pub fn accept_ok(event: Result<E, R>) -> Self {
        match event {
            Ok(e) => Self::accept(e),
            Err(r) => Self::reject(r),
        }
    }

    /// Constructs a program that accepts an event or rejects with several
    /// reasons.
    pub fn accept_ok_nec(event: Result<E, NonEmpty<R>>) -> Self {
        match event {
            Ok(e) => Self::accept(e),
            Err(rs) => Decision::Rejected(rs),
        }
    }

    /// Constructs a program that rejects with a sequence of reasons when
    /// `predicate` holds, or does nothing otherwise.
    pub fn reject_when(predicate: bool, reasons: impl Into<NonEmpty<R>>) -> Self {
        if predicate {
            Self::reject(reasons)
        } else {
            Self::unit()
        }
    }

    /// Constructs a program that rejects with an optional single reason or
    /// does nothing otherwise.
    pub fn reject_some(reason: Option<R>) -> Self {
        match reason {
            Some(r) => Self::reject(r),
            None => Self::unit(),
        }
    }
}

impl<R, E, A> Decision<R, E, Option<A>> {
    /// Turns a decision of an optional output into an optional decision,
    /// keeping events and rejections (the `Traverse` instance for `Option`).
    pub fn transpose(self) -> Option<Decision<R, E, A>> {
        match self {
            Decision::InDecisive(None) | Decision::Accepted { result: None, .. } => None,
            Decision::InDecisive(Some(a)) => Some(Decision::InDecisive(a)),
            Decision::Accepted {
                events,
                result: Some(a),
            } => Some(Decision::Accepted { events, result: a }),
            Decision::Rejected(reasons) => Some(Decision::Rejected(reasons)),
        }
    }
}

impl<R, E, A, X> Decision<R, E, Result<A, X>> {
    /// Turns a decision of a fallible output into a fallible decision,
    /// keeping events and rejections (the `Traverse` instance for `Result`).
    pub fn transpose(self) -> Result<Decision<R, E, A>, X> {
        match self {
            Decision::InDecisive(Err(x)) | Decision::Accepted { result: Err(x), .. } => Err(x),
            Decision::InDecisive(Ok(a)) => Ok(Decision::InDecisive(a)),
            Decision::Accepted {
                events,
                result: Ok(a),
            } => Ok(Decision::Accepted { events, result: a }),
            Decision::Rejected(reasons) => Ok(Decision::Rejected(reasons)),
        }
    }
}

/// Sequences an iterator of decisions into a decision of a collection
/// (`traverse`/`sequence`): events accumulate in order and the first
/// rejection short-circuits.
///
/// ```
/// use edomata_core::{Decision, nonempty};
///
/// let ds = vec![Decision::accept_return(1, "a"), Decision::pure(2), Decision::accept_return(3, "b")];
/// let all: Decision<(), &str, Vec<i32>> = ds.into_iter().collect();
/// assert_eq!(all, Decision::Accepted { events: nonempty!["a", "b"], result: vec![1, 2, 3] });
/// ```
impl<R, E, A, C: Default + Extend<A>> FromIterator<Decision<R, E, A>> for Decision<R, E, C> {
    fn from_iter<I: IntoIterator<Item = Decision<R, E, A>>>(iter: I) -> Self {
        let mut events: Vec<E> = Vec::new();
        let mut out = C::default();
        for d in iter {
            match d {
                Decision::InDecisive(a) => out.extend([a]),
                Decision::Accepted {
                    events: evs,
                    result,
                } => {
                    events.extend(evs);
                    out.extend([result]);
                }
                Decision::Rejected(reasons) => return Decision::Rejected(reasons),
            }
        }
        match NonEmpty::from_vec(events) {
            None => Decision::InDecisive(out),
            Some(events) => Decision::Accepted {
                events,
                result: out,
            },
        }
    }
}

impl<R, E, A> From<Result<A, NonEmpty<R>>> for Decision<R, E, A> {
    fn from(result: Result<A, NonEmpty<R>>) -> Self {
        Decision::validate(result)
    }
}
