//! Domain models with plain `Result` transitions, the counterpart of
//! `JDomainModel`.

use std::sync::Arc;

use edomata_core::{DomainModel, NonEmpty};

/// A domain model whose transition returns a plain `Result` with a vector
/// of rejections. Mirrors Scala's `JDomainModel`; implement it directly or
/// build one from closures with [`ClosureModel::new`] (`JDomainModel.create`).
pub trait SimpleDomainModel: Send + Sync + 'static {
    /// Aggregate state.
    type State;
    /// Domain event.
    type Event;
    /// Domain error.
    type Rejection;

    /// Initial (empty) state for this aggregate.
    fn initial(&self) -> Self::State;

    /// Given an event and the current state, the new state or the
    /// rejections. As in Scala, an `Err` must carry at least one reason;
    /// the adapter to the core model panics otherwise
    /// (`IllegalArgumentException` in Scala).
    fn transition(
        &self,
        event: &Self::Event,
        state: Self::State,
    ) -> Result<Self::State, Vec<Self::Rejection>>;

    /// Adapts this model to the core [`DomainModel`] trait
    /// (`JDomainModel.toModelTC`).
    fn into_model(self) -> ModelAdapter<Self>
    where
        Self: Sized,
    {
        ModelAdapter(self)
    }
}

type TransitionFn<S, E, R> = dyn Fn(&E, S) -> Result<S, Vec<R>> + Send + Sync;

/// A [`SimpleDomainModel`] built from closures.
pub struct ClosureModel<S, E, R> {
    initial: S,
    transition: Arc<TransitionFn<S, E, R>>,
}

impl<S: Clone, E, R> Clone for ClosureModel<S, E, R> {
    fn clone(&self) -> Self {
        Self {
            initial: self.initial.clone(),
            transition: Arc::clone(&self.transition),
        }
    }
}

impl<S: std::fmt::Debug, E, R> std::fmt::Debug for ClosureModel<S, E, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClosureModel")
            .field("initial", &self.initial)
            .finish()
    }
}

impl<S, E, R> ClosureModel<S, E, R>
where
    S: Clone + Send + Sync + 'static,
    E: Send + Sync + 'static,
    R: Send + Sync + 'static,
{
    /// Builds a model from an initial state and a transition closure
    /// (`JDomainModel.create`).
    pub fn new<F>(initial: S, transition: F) -> Self
    where
        F: Fn(&E, S) -> Result<S, Vec<R>> + Send + Sync + 'static,
    {
        Self {
            initial,
            transition: Arc::new(transition),
        }
    }
}

impl<S, E, R> SimpleDomainModel for ClosureModel<S, E, R>
where
    S: Clone + Send + Sync + 'static,
    E: Send + Sync + 'static,
    R: Send + Sync + 'static,
{
    type State = S;
    type Event = E;
    type Rejection = R;

    fn initial(&self) -> S {
        self.initial.clone()
    }

    fn transition(&self, event: &E, state: S) -> Result<S, Vec<R>> {
        (self.transition)(event, state)
    }
}

/// A [`SimpleDomainModel`] seen as a core [`DomainModel`].
#[derive(Clone, Debug)]
pub struct ModelAdapter<M>(pub M);

impl<M: SimpleDomainModel> DomainModel for ModelAdapter<M> {
    type State = M::State;
    type Event = M::Event;
    type Rejection = M::Rejection;

    fn initial(&self) -> M::State {
        self.0.initial()
    }

    fn transition(
        &self,
        event: &M::Event,
        state: M::State,
    ) -> Result<M::State, NonEmpty<M::Rejection>> {
        self.0.transition(event, state).map_err(|reasons| {
            NonEmpty::from_vec(reasons).expect("Rejection must have at least one reason")
        })
    }
}
