//! Domain model definitions.

use crate::{CqrsDomainDsl, Decision, DomainDsl, NonEmpty};

/// Definition of an event-sourced domain model: how the aggregate state
/// starts and how it transitions in response to events.
///
/// This merges Scala's `DomainModel` and `ModelTC`.
///
/// # Type parameters
///
/// * `State` – state model of your program, a.k.a. aggregate root
/// * `Event` – domain events
/// * `Rejection` – domain error type
///
/// ```
/// use edomata_core::{Decision, DomainModel, NonEmpty, nonempty};
///
/// struct Counter;
///
/// impl DomainModel for Counter {
///     type State = i32;
///     type Event = i32;
///     type Rejection = &'static str;
///
///     fn initial(&self) -> i32 { 0 }
///
///     fn transition(&self, event: &i32, state: i32) -> Result<i32, NonEmpty<&'static str>> {
///         if *event > 0 { Ok(state + event) } else { Err(NonEmpty::new("Invalid number!")) }
///     }
/// }
///
/// assert_eq!(Counter.accept(1, 2), Decision::accept_return(3, 2));
/// assert_eq!(Counter.accept(1, nonempty![2, 3]), Decision::accept_return(6, nonempty![2, 3]));
/// assert!(Counter.accept(1, -1).is_rejected());
/// ```
pub trait DomainModel {
    /// Aggregate state.
    type State;
    /// Domain event.
    type Event;
    /// Domain error.
    type Rejection;

    /// Initial or empty value for this domain model.
    ///
    /// Any aggregate is either created and has a history, or is in the
    /// initial state.
    fn initial(&self) -> Self::State;

    /// Defines how this model transitions in response to an event. It is
    /// like an event handler, but pure.
    ///
    /// An event that can't be applied results in a rejection or a conflict,
    /// based on whether it is read from the journal or applied for a
    /// decision.
    fn transition(
        &self,
        event: &Self::Event,
        state: Self::State,
    ) -> Result<Self::State, NonEmpty<Self::Rejection>>;

    /// Applies all events in order, returning the new state or the first
    /// rejection.
    fn apply_all<'a, I>(
        &self,
        state: Self::State,
        events: I,
    ) -> Result<Self::State, NonEmpty<Self::Rejection>>
    where
        Self::Event: 'a,
        I: IntoIterator<Item = &'a Self::Event>,
    {
        events
            .into_iter()
            .try_fold(state, |s, e| self.transition(e, s))
    }

    /// Like [`DomainModel::perform`], but also returns the decision output.
    fn handle<T>(
        &self,
        state: Self::State,
        decision: Decision<Self::Rejection, Self::Event, T>,
    ) -> Decision<Self::Rejection, Self::Event, (Self::State, T)> {
        match decision {
            Decision::Accepted { events, result } => match self.apply_all(state, events.iter()) {
                Ok(s) => Decision::Accepted {
                    events,
                    result: (s, result),
                },
                Err(reasons) => Decision::Rejected(reasons),
            },
            Decision::InDecisive(v) => Decision::pure((state, v)),
            Decision::Rejected(reasons) => Decision::Rejected(reasons),
        }
    }

    /// Returns a decision that has applied this decision and folded the
    /// state, so the output is the new state.
    fn perform<T>(
        &self,
        state: Self::State,
        decision: Decision<Self::Rejection, Self::Event, T>,
    ) -> Decision<Self::Rejection, Self::Event, Self::State> {
        self.handle(state, decision.void()).map(|(s, ())| s)
    }

    /// Helps with deciding based on the state and then applying the
    /// decision.
    fn decide<T, F>(
        &self,
        state: Self::State,
        f: F,
    ) -> Decision<Self::Rejection, Self::Event, Self::State>
    where
        F: FnOnce(&Self::State) -> Decision<Self::Rejection, Self::Event, T>,
    {
        let decision = f(&state);
        self.perform(state, decision)
    }

    /// Helps with deciding based on the state and then applying the
    /// decision, also returning the output.
    fn decide_return<T, F>(
        &self,
        state: Self::State,
        f: F,
    ) -> Decision<Self::Rejection, Self::Event, (Self::State, T)>
    where
        F: FnOnce(&Self::State) -> Decision<Self::Rejection, Self::Event, T>,
    {
        let decision = f(&state);
        self.handle(state, decision)
    }

    /// Applies events to the state, returning the new state.
    fn accept(
        &self,
        state: Self::State,
        events: impl Into<NonEmpty<Self::Event>>,
    ) -> Decision<Self::Rejection, Self::Event, Self::State> {
        self.perform(state, Decision::accept(events))
    }

    /// A DSL for writing programs on this model with commands `C` and
    /// notifications `N`.
    fn dsl<C, N>(&self) -> DomainDsl<C, Self::State, Self::Event, Self::Rejection, N> {
        DomainDsl::new()
    }
}

/// Definition of a CQRS domain model: only an initial state, since the state
/// is stored directly.
///
/// This merges Scala's `CQRSModel` and `StateModelTC`.
pub trait CqrsModel {
    /// Aggregate state.
    type State;
    /// Domain error.
    type Rejection;

    /// Initial or empty value for this domain model.
    fn initial(&self) -> Self::State;

    /// A DSL for writing programs on this model with commands `C` and
    /// notifications `N`.
    fn dsl<C, N>(&self) -> CqrsDomainDsl<C, Self::State, Self::Rejection, N> {
        CqrsDomainDsl::new()
    }
}
