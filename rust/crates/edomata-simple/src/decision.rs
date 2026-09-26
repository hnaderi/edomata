//! A plain-data decision type, the counterpart of `JDecision`.

use edomata_core::{Decision, NonEmpty};

/// The outcome of handling a command, with plain vectors instead of
/// non-empty chains. Mirrors Scala's `JDecision`: an `Accepted` decision
/// carries events and a result, `Rejected` carries reasons and
/// `Indecisive` only a result.
///
/// Unlike [`Decision`], the vectors may be empty; conversion to the core
/// type treats an `Accepted` decision without events as `Indecisive` and
/// refuses a `Rejected` decision without reasons (see
/// [`SimpleDecision::into_decision`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SimpleDecision<R, E, A> {
    /// The command was accepted: `events` are appended and `result` is
    /// produced.
    Accepted {
        /// Events to append (never empty when built with the constructors).
        events: Vec<E>,
        /// Result of the decision.
        result: A,
    },
    /// The command was rejected.
    Rejected {
        /// Reasons of the rejection.
        reasons: Vec<R>,
    },
    /// No event was produced; `result` is produced.
    Indecisive {
        /// Result of the decision.
        result: A,
    },
}

/// Error of [`SimpleDecision::into_decision`]: a rejection without reasons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("Rejected must have at least one reason")]
pub struct EmptyRejection;

impl<R, E, A> SimpleDecision<R, E, A> {
    /// Accepts the given events with a `()` result (`JDecision.accept`).
    pub fn accept<I: IntoIterator<Item = E>>(events: I) -> SimpleDecision<R, E, ()> {
        SimpleDecision::Accepted {
            events: events.into_iter().collect(),
            result: (),
        }
    }

    /// Accepts the given events with a result (`JDecision.acceptReturn`).
    pub fn accept_return<I: IntoIterator<Item = E>>(result: A, events: I) -> Self {
        SimpleDecision::Accepted {
            events: events.into_iter().collect(),
            result,
        }
    }

    /// Rejects with the given reasons (`JDecision.reject`).
    pub fn reject<I: IntoIterator<Item = R>>(reasons: I) -> Self {
        SimpleDecision::Rejected {
            reasons: reasons.into_iter().collect(),
        }
    }

    /// A decision without events (`JDecision.pure`).
    pub fn pure(result: A) -> Self {
        SimpleDecision::Indecisive { result }
    }

    /// A decision without events nor result (`JDecision.unit`).
    pub fn unit() -> SimpleDecision<R, E, ()> {
        SimpleDecision::Indecisive { result: () }
    }

    /// Whether the decision is `Accepted`.
    pub fn is_accepted(&self) -> bool {
        matches!(self, SimpleDecision::Accepted { .. })
    }

    /// Whether the decision is `Rejected`.
    pub fn is_rejected(&self) -> bool {
        matches!(self, SimpleDecision::Rejected { .. })
    }

    /// Whether the decision is `Indecisive`.
    pub fn is_indecisive(&self) -> bool {
        matches!(self, SimpleDecision::Indecisive { .. })
    }

    /// Transforms the result, keeping events and rejections.
    pub fn map<B, F: FnOnce(A) -> B>(self, f: F) -> SimpleDecision<R, E, B> {
        match self {
            SimpleDecision::Accepted { events, result } => SimpleDecision::Accepted {
                events,
                result: f(result),
            },
            SimpleDecision::Rejected { reasons } => SimpleDecision::Rejected { reasons },
            SimpleDecision::Indecisive { result } => {
                SimpleDecision::Indecisive { result: f(result) }
            }
        }
    }

    /// Chains a decision on the result (`JDecision.flatMap`): events are
    /// merged in order, a rejection short-circuits.
    pub fn and_then<B, F: FnOnce(A) -> SimpleDecision<R, E, B>>(
        self,
        f: F,
    ) -> SimpleDecision<R, E, B> {
        match self {
            SimpleDecision::Accepted { mut events, result } => match f(result) {
                SimpleDecision::Accepted {
                    events: more,
                    result,
                } => {
                    events.extend(more);
                    SimpleDecision::Accepted { events, result }
                }
                SimpleDecision::Rejected { reasons } => SimpleDecision::Rejected { reasons },
                SimpleDecision::Indecisive { result } => {
                    SimpleDecision::Accepted { events, result }
                }
            },
            SimpleDecision::Rejected { reasons } => SimpleDecision::Rejected { reasons },
            SimpleDecision::Indecisive { result } => f(result),
        }
    }

    /// Alias of [`SimpleDecision::and_then`].
    pub fn flat_map<B, F: FnOnce(A) -> SimpleDecision<R, E, B>>(
        self,
        f: F,
    ) -> SimpleDecision<R, E, B> {
        self.and_then(f)
    }

    /// The result, or the rejection reasons (`JDecision.toEither`).
    pub fn to_result(self) -> Result<A, Vec<R>> {
        match self {
            SimpleDecision::Accepted { result, .. } | SimpleDecision::Indecisive { result } => {
                Ok(result)
            }
            SimpleDecision::Rejected { reasons } => Err(reasons),
        }
    }

    /// The events of an accepted decision (empty otherwise).
    pub fn events(&self) -> &[E] {
        match self {
            SimpleDecision::Accepted { events, .. } => events,
            _ => &[],
        }
    }

    /// The reasons of a rejected decision (empty otherwise).
    pub fn reasons(&self) -> &[R] {
        match self {
            SimpleDecision::Rejected { reasons } => reasons,
            _ => &[],
        }
    }

    /// Converts to the core [`Decision`] (`Converters.decisionToScala`):
    /// an `Accepted` decision without events becomes `InDecisive`; a
    /// `Rejected` decision without reasons is an error.
    pub fn into_decision(self) -> Result<Decision<R, E, A>, EmptyRejection> {
        match self {
            SimpleDecision::Accepted { events, result } => Ok(match NonEmpty::from_vec(events) {
                Some(events) => Decision::Accepted { events, result },
                None => Decision::InDecisive(result),
            }),
            SimpleDecision::Rejected { reasons } => NonEmpty::from_vec(reasons)
                .map(Decision::Rejected)
                .ok_or(EmptyRejection),
            SimpleDecision::Indecisive { result } => Ok(Decision::InDecisive(result)),
        }
    }
}

impl<R, E, A> From<Decision<R, E, A>> for SimpleDecision<R, E, A> {
    /// `Converters.decisionToJava`.
    fn from(d: Decision<R, E, A>) -> Self {
        match d {
            Decision::Accepted { events, result } => SimpleDecision::Accepted {
                events: events.into_vec(),
                result,
            },
            Decision::Rejected(reasons) => SimpleDecision::Rejected {
                reasons: reasons.into_vec(),
            },
            Decision::InDecisive(result) => SimpleDecision::Indecisive { result },
        }
    }
}

impl<R, E, A> TryFrom<SimpleDecision<R, E, A>> for Decision<R, E, A> {
    type Error = EmptyRejection;

    fn try_from(d: SimpleDecision<R, E, A>) -> Result<Self, EmptyRejection> {
        d.into_decision()
    }
}
