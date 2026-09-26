//! Extension traits that turn plain values, `Option`s and `Result`s into
//! decisions.
//!
//! ```
//! use edomata_core::Decision;
//! use edomata_core::syntax::*;
//!
//! assert_eq!(1.accept(), Decision::<(), _, ()>::accept(1));
//! assert_eq!(Some(1).to_decision("missing"), Decision::<_, (), _>::pure(1));
//! assert_eq!(None::<i32>.to_decision("missing"), Decision::<_, (), i32>::reject("missing"));
//! ```

use crate::{Decision, NonEmpty};

/// Turns any value into a decision.
pub trait ValueDecisionExt: Sized {
    /// A decision that outputs this value.
    fn into_decision<R, E>(self) -> Decision<R, E, Self> {
        Decision::pure(self)
    }

    /// A decision that accepts this value as an event.
    fn accept<R>(self) -> Decision<R, Self, ()> {
        Decision::accept(self)
    }

    /// A decision that rejects with this value as the reason.
    fn reject<E, A>(self) -> Decision<Self, E, A> {
        Decision::reject(self)
    }
}

impl<T> ValueDecisionExt for T {}

/// Turns an `Option` into a decision.
pub trait OptionDecisionExt<T> {
    /// Outputs the value if present, rejects otherwise.
    fn to_decision<R, E>(self, or_else: impl Into<NonEmpty<R>>) -> Decision<R, E, T>;
    /// Accepts the value as an event if present, does nothing otherwise.
    fn to_accepted<R>(self) -> Decision<R, T, ()>;
    /// Accepts the value as an event if present, rejects otherwise.
    fn to_accepted_or<R>(self, or_else: impl Into<NonEmpty<R>>) -> Decision<R, T, ()>;
    /// Rejects with the value as the reason if present, does nothing
    /// otherwise.
    fn to_rejected<E>(self) -> Decision<T, E, ()>;
}

impl<T> OptionDecisionExt<T> for Option<T> {
    fn to_decision<R, E>(self, or_else: impl Into<NonEmpty<R>>) -> Decision<R, E, T> {
        Decision::from_option(self, or_else)
    }

    fn to_accepted<R>(self) -> Decision<R, T, ()> {
        Decision::accept_some(self)
    }

    fn to_accepted_or<R>(self, or_else: impl Into<NonEmpty<R>>) -> Decision<R, T, ()> {
        Decision::accept_some_or(self, or_else)
    }

    fn to_rejected<E>(self) -> Decision<T, E, ()> {
        Decision::reject_some(self)
    }
}

/// Turns a `Result` with a single error into a decision.
pub trait ResultDecisionExt<T, R> {
    /// Outputs the value or rejects with the error.
    fn to_decision<E>(self) -> Decision<R, E, T>;
    /// Accepts the value as an event or rejects with the error.
    fn to_accepted(self) -> Decision<R, T, ()>;
}

impl<T, R> ResultDecisionExt<T, R> for Result<T, R> {
    fn to_decision<E>(self) -> Decision<R, E, T> {
        Decision::from_result(self)
    }

    fn to_accepted(self) -> Decision<R, T, ()> {
        Decision::accept_ok(self)
    }
}

/// Turns a `Result` with several errors into a decision.
pub trait ResultNecDecisionExt<T, R> {
    /// Outputs the value or rejects with all the errors.
    fn to_decision_nec<E>(self) -> Decision<R, E, T>;
    /// Accepts the value as an event or rejects with all the errors.
    fn to_accepted_nec(self) -> Decision<R, T, ()>;
}

impl<T, R> ResultNecDecisionExt<T, R> for Result<T, NonEmpty<R>> {
    fn to_decision_nec<E>(self) -> Decision<R, E, T> {
        Decision::validate(self)
    }

    fn to_accepted_nec(self) -> Decision<R, T, ()> {
        Decision::accept_ok_nec(self)
    }
}
