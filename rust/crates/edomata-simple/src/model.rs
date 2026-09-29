//! Domain models with plain `Result` transitions, the counterpart of
//! `JDomainModel`.

use std::sync::Arc;

use edomata_core::{DomainModel, NonEmpty};

/// A domain model whose transition returns a plain `Result` with a vector
/// of rejections. Mirrors Scala's `JDomainModel`; implement it directly or
/// build one from closures with [`ClosureModel::new`] (`JDomainModel.create`).
///
/// ```
/// use edomata_simple::SimpleDomainModel;
///
/// enum Event { Deposited(u64), Withdrawn(u64) }
///
/// struct Account;
///
/// impl SimpleDomainModel for Account {
///     type State = u64;
///     type Event = Event;
///     type Rejection = String;
///
///     fn initial(&self) -> u64 { 0 }
///
///     fn transition(&self, event: &Event, balance: u64) -> Result<u64, Vec<String>> {
///         match event {
///             Event::Deposited(n) => Ok(balance + n),
///             Event::Withdrawn(n) => balance.checked_sub(*n).ok_or_else(|| vec!["insufficient balance".to_string()]),
///         }
///     }
/// }
///
/// assert_eq!(Account.transition(&Event::Deposited(10), 0), Ok(10));
/// assert!(Account.transition(&Event::Withdrawn(10), 0).is_err());
/// ```
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
    /// rejections. As in Scala, an `Err` must carry at least one reason:
    /// [`ModelAdapter`] (and so the backend) panics on an empty `Err`
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
///
/// ```
/// use edomata_simple::{ClosureModel, SimpleDomainModel};
///
/// let model = <ClosureModel<i64, i64, String>>::new(0, |event: &i64, balance| {
///     let next = balance + event;
///     if next < 0 { Err(vec!["insufficient balance".to_string()]) } else { Ok(next) }
/// });
/// assert_eq!(model.initial(), 0);
/// assert_eq!(model.transition(&5, 10), Ok(15));
/// assert_eq!(model.transition(&-20, 10), Err(vec!["insufficient balance".to_string()]));
/// ```
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

/// A [`SimpleDomainModel`] seen as a core [`DomainModel`]: rejection vectors
/// become [`NonEmpty`] chains.
///
/// # Panics
///
/// Its `transition` panics if the wrapped model returns `Err` with an empty
/// vector, a programming error (Scala throws `IllegalArgumentException`).
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
