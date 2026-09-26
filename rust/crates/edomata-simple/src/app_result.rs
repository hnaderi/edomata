//! The output of a command handler, the counterpart of `JAppResult`.

use crate::SimpleDecision;

/// What a command handler returns: a decision (with a `()` result) and the
/// notifications to publish. Mirrors Scala's `JAppResult`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppResult<R, E, N> {
    /// The decision.
    pub decision: SimpleDecision<R, E, ()>,
    /// Notifications to publish, whatever the decision.
    pub notifications: Vec<N>,
}

impl<R, E, N> AppResult<R, E, N> {
    /// A result with a decision and no notification (`JAppResult.decide`).
    pub fn decide(decision: SimpleDecision<R, E, ()>) -> Self {
        Self {
            decision,
            notifications: Vec::new(),
        }
    }

    /// A result with a decision and notifications
    /// (`JAppResult.decideAndPublish`).
    pub fn decide_and_publish<I: IntoIterator<Item = N>>(
        decision: SimpleDecision<R, E, ()>,
        notifications: I,
    ) -> Self {
        Self {
            decision,
            notifications: notifications.into_iter().collect(),
        }
    }

    /// Accepts events (`JAppResult.accept`).
    pub fn accept<I: IntoIterator<Item = E>>(events: I) -> Self {
        Self::decide(SimpleDecision::<R, E, ()>::accept(events))
    }

    /// Rejects (`JAppResult.reject`).
    pub fn reject<I: IntoIterator<Item = R>>(reasons: I) -> Self {
        Self::decide(SimpleDecision::reject(reasons))
    }

    /// Publishes notifications without deciding anything
    /// (`JAppResult.publish`).
    pub fn publish<I: IntoIterator<Item = N>>(notifications: I) -> Self {
        Self::decide_and_publish(SimpleDecision::<R, E, ()>::unit(), notifications)
    }

    /// Adds notifications to this result.
    pub fn and_publish<I: IntoIterator<Item = N>>(mut self, notifications: I) -> Self {
        self.notifications.extend(notifications);
        self
    }
}
