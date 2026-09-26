//! Event-sourcing backend: journal, snapshots, repositories and the command
//! handler for [`Edomaton`](edomata_core::Edomaton) programs.

mod backend;
mod cached_repository;
mod command_handler;
mod journal;
mod notifications;
mod repository;
mod repository_reader;
mod snapshot;
mod storage;

pub use backend::{Backend, BackendBuilder, PartialBackendBuilder};
pub use cached_repository::CachedRepository;
pub use command_handler::CommandHandler;
pub use journal::JournalReader;
pub use notifications::{Notifications, NotificationsConsumer, NotificationsPublisher};
pub use repository::Repository;
pub use repository_reader::{JournalRepositoryReader, RepositoryReader};
pub use snapshot::{
    InMemorySnapshotStore, PersistedSnapshotConfig, PersistedSnapshotStore, SnapshotItem,
    SnapshotPersistence, SnapshotReader, SnapshotStore, as_reader, dedup,
};
pub use storage::{Storage, StorageDriver};

use edomata_core::NonEmpty;

use crate::{EventMessage, SeqNr};

/// A folded aggregate: its state and the number of events applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidState<S> {
    /// Current state.
    pub state: S,
    /// Number of events applied so far (the version).
    pub version: SeqNr,
}

impl<S> ValidState<S> {
    /// Builds a valid aggregate state.
    pub fn new(state: S, version: SeqNr) -> Self {
        Self { state, version }
    }
}

/// State of an aggregate as read from the journal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AggregateState<S, E, R> {
    /// Every journaled event could be applied.
    Valid(ValidState<S>),
    /// A journaled event could not be applied: the model and the journal
    /// disagree.
    Conflicted {
        /// Last valid state.
        last: S,
        /// The event that could not be applied.
        on_event: EventMessage<E>,
        /// Why the model refused it.
        errors: NonEmpty<R>,
    },
}

impl<S, E, R> AggregateState<S, E, R> {
    /// Builds a valid aggregate state.
    pub fn valid(state: S, version: SeqNr) -> Self {
        AggregateState::Valid(ValidState { state, version })
    }

    /// Whether this state is valid.
    pub fn is_valid(&self) -> bool {
        matches!(self, AggregateState::Valid(_))
    }

    /// The valid state, if any.
    pub fn as_valid(&self) -> Option<&ValidState<S>> {
        match self {
            AggregateState::Valid(v) => Some(v),
            AggregateState::Conflicted { .. } => None,
        }
    }
}

impl<S, E, R> From<ValidState<S>> for AggregateState<S, E, R> {
    fn from(v: ValidState<S>) -> Self {
        AggregateState::Valid(v)
    }
}

/// What a repository knows about a command before handling it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandState<S, E, R> {
    /// The command was already handled; nothing to do.
    Redundant,
    /// The command is new; here is the aggregate it targets.
    Aggregate(AggregateState<S, E, R>),
}

impl<S, E, R> From<AggregateState<S, E, R>> for CommandState<S, E, R> {
    fn from(a: AggregateState<S, E, R>) -> Self {
        CommandState::Aggregate(a)
    }
}

impl<S, E, R> From<ValidState<S>> for CommandState<S, E, R> {
    fn from(v: ValidState<S>) -> Self {
        CommandState::Aggregate(AggregateState::Valid(v))
    }
}
