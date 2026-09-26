//! CQRS backend: state storage, repositories and the command handler for
//! [`Stomaton`](edomata_core::Stomaton) programs (no event sourcing).

mod backend;
mod cached_repository;
mod command_handler;
mod notifications;
mod repository;
mod storage;

pub use backend::{Backend, BackendBuilder, PartialBackendBuilder};
pub use cached_repository::CachedRepository;
pub use command_handler::CommandHandler;
pub use notifications::{Notifications, NotificationsConsumer, NotificationsPublisher};
pub use repository::{Repository, RepositoryReader};
pub use storage::{Storage, StorageDriver};

use std::sync::Arc;

use edomata_core::CqrsModel;

use crate::SeqNr;

/// State of an aggregate as stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AggregateState<S> {
    /// Current state.
    pub state: S,
    /// Number of times the state was saved (the version).
    pub version: SeqNr,
}

impl<S> AggregateState<S> {
    /// Builds an aggregate state.
    pub fn new(state: S, version: SeqNr) -> Self {
        Self { state, version }
    }
}

/// What a repository knows about a command before handling it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandState<S> {
    /// The command was already handled; nothing to do.
    Redundant,
    /// The command is new; here is the aggregate it targets.
    Aggregate(AggregateState<S>),
}

impl<S> From<AggregateState<S>> for CommandState<S> {
    fn from(a: AggregateState<S>) -> Self {
        CommandState::Aggregate(a)
    }
}

/// The part of a [`CqrsModel`] a storage needs: the initial state. Mirrors
/// Scala's `StateModelTC`.
pub trait StateModel<S>: Send + Sync {
    /// Initial state of an aggregate that was never saved.
    fn initial(&self) -> S;
}

impl<M> StateModel<M::State> for M
where
    M: CqrsModel + Send + Sync,
{
    fn initial(&self) -> M::State {
        CqrsModel::initial(self)
    }
}

/// A shared, object-safe state model.
pub type SharedStateModel<S> = Arc<dyn StateModel<S>>;
