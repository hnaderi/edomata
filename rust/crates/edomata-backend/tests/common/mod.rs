//! Test doubles: port of `Doubles.scala`, `eventsourcing/Doubles.scala`,
//! `eventsourcing/FakeRepository.scala`, `cqrs/FakeRepository.scala` and
//! `FakeOutboxReader.scala`.

#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use edomata_backend::cqrs;
use edomata_backend::eventsourcing::{
    AggregateState, CommandState, JournalReader, Repository, SnapshotItem, SnapshotPersistence,
    SnapshotReader, SnapshotStore, ValidState,
};
use edomata_backend::{
    BackendError, BoxStream, CommandRef, CommandStore, EventMessage, EventVersion, OutboxItem,
    OutboxReader, Payload, SeqNr, StreamId,
};
use edomata_core::{CommandMessage, NonEmpty};

pub fn planned_failure() -> BackendError {
    BackendError::PersistenceError("planned failure".into())
}

pub fn max_time() -> DateTime<Utc> {
    DateTime::<Utc>::MAX_UTC
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

// ---------------------------------------------------------------------------
// Command stores
// ---------------------------------------------------------------------------

pub struct BlackHoleCommandStore;

#[async_trait]
impl CommandStore for BlackHoleCommandStore {
    async fn append(&self, _id: &str) -> Result<(), BackendError> {
        Ok(())
    }
    async fn contains(&self, _id: &str) -> Result<bool, BackendError> {
        Ok(false)
    }
}

pub struct YesManCommandStore;

#[async_trait]
impl CommandStore for YesManCommandStore {
    async fn append(&self, _id: &str) -> Result<(), BackendError> {
        Ok(())
    }
    async fn contains(&self, _id: &str) -> Result<bool, BackendError> {
        Ok(true)
    }
}

#[derive(Default)]
pub struct FakeCommandStore {
    states: Mutex<HashSet<String>>,
}

impl FakeCommandStore {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn all(&self) -> HashSet<String> {
        lock(&self.states).clone()
    }
}

#[async_trait]
impl CommandStore for FakeCommandStore {
    async fn append(&self, id: &str) -> Result<(), BackendError> {
        lock(&self.states).insert(id.to_owned());
        Ok(())
    }
    async fn contains(&self, id: &str) -> Result<bool, BackendError> {
        Ok(lock(&self.states).contains(id))
    }
}

// ---------------------------------------------------------------------------
// Snapshot stores
// ---------------------------------------------------------------------------

pub struct BlackHoleSnapshotStore;

#[async_trait]
impl<S: Payload> SnapshotReader<S> for BlackHoleSnapshotStore {
    async fn get(&self, _id: &str) -> Result<Option<ValidState<S>>, BackendError> {
        Ok(None)
    }
    async fn get_fast(&self, _id: &str) -> Option<ValidState<S>> {
        None
    }
}

#[async_trait]
impl<S: Payload> SnapshotStore<S> for BlackHoleSnapshotStore {
    async fn put(&self, _id: &str, _state: ValidState<S>) -> Result<(), BackendError> {
        Ok(())
    }
}

pub struct ConstantSnapshotStore<S> {
    pub state: S,
    pub version: SeqNr,
}

#[async_trait]
impl<S: Payload> SnapshotReader<S> for ConstantSnapshotStore<S> {
    async fn get(&self, _id: &str) -> Result<Option<ValidState<S>>, BackendError> {
        Ok(Some(ValidState::new(self.state.clone(), self.version)))
    }
    async fn get_fast(&self, _id: &str) -> Option<ValidState<S>> {
        Some(ValidState::new(self.state.clone(), self.version))
    }
}

#[async_trait]
impl<S: Payload> SnapshotStore<S> for ConstantSnapshotStore<S> {
    async fn put(&self, _id: &str, _state: ValidState<S>) -> Result<(), BackendError> {
        Ok(())
    }
}

pub struct LaggedSnapshotStore<S> {
    pub state: S,
    pub version: SeqNr,
    pub lagged: SeqNr,
}

#[async_trait]
impl<S: Payload> SnapshotReader<S> for LaggedSnapshotStore<S> {
    async fn get(&self, _id: &str) -> Result<Option<ValidState<S>>, BackendError> {
        Ok(Some(ValidState::new(self.state.clone(), self.lagged)))
    }
    async fn get_fast(&self, _id: &str) -> Option<ValidState<S>> {
        Some(ValidState::new(self.state.clone(), self.version))
    }
}

#[async_trait]
impl<S: Payload> SnapshotStore<S> for LaggedSnapshotStore<S> {
    async fn put(&self, _id: &str, _state: ValidState<S>) -> Result<(), BackendError> {
        Ok(())
    }
}

#[derive(Default)]
pub struct FakeSnapshotStore<S> {
    states: Mutex<HashMap<StreamId, ValidState<S>>>,
}

impl<S: Clone> FakeSnapshotStore<S> {
    pub fn new() -> Self {
        Self {
            states: Mutex::new(HashMap::new()),
        }
    }
    pub fn all(&self) -> HashMap<StreamId, ValidState<S>> {
        lock(&self.states).clone()
    }
}

#[async_trait]
impl<S: Payload> SnapshotReader<S> for FakeSnapshotStore<S> {
    async fn get(&self, id: &str) -> Result<Option<ValidState<S>>, BackendError> {
        Ok(lock(&self.states).get(id).cloned())
    }
    async fn get_fast(&self, id: &str) -> Option<ValidState<S>> {
        lock(&self.states).get(id).cloned()
    }
}

#[async_trait]
impl<S: Payload> SnapshotStore<S> for FakeSnapshotStore<S> {
    async fn put(&self, id: &str, state: ValidState<S>) -> Result<(), BackendError> {
        lock(&self.states).insert(id.to_owned(), state);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Snapshot persistence
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct FakeSnapshotPersistence<S> {
    items: Mutex<HashMap<StreamId, ValidState<S>>>,
}

impl<S: Clone> FakeSnapshotPersistence<S> {
    pub fn new() -> Self {
        Self {
            items: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl<S: Payload> SnapshotPersistence<S> for FakeSnapshotPersistence<S> {
    async fn get(&self, id: &str) -> Result<Option<ValidState<S>>, BackendError> {
        Ok(lock(&self.items).get(id).cloned())
    }
    async fn put(&self, items: Vec<SnapshotItem<S>>) -> Result<(), BackendError> {
        lock(&self.items).extend(items);
        Ok(())
    }
}

/// Fails the first `required_failures` operations, then delegates.
pub struct FailingSnapshotPersistence<S> {
    inner: FakeSnapshotPersistence<S>,
    failures: AtomicUsize,
    required_failures: usize,
}

impl<S: Clone> FailingSnapshotPersistence<S> {
    pub fn new(required_failures: usize) -> Self {
        Self {
            inner: FakeSnapshotPersistence::new(),
            failures: AtomicUsize::new(0),
            required_failures,
        }
    }

    fn should_fail(&self) -> bool {
        if self.failures.load(Ordering::SeqCst) < self.required_failures {
            self.failures.fetch_add(1, Ordering::SeqCst);
            true
        } else {
            false
        }
    }
}

#[async_trait]
impl<S: Payload> SnapshotPersistence<S> for FailingSnapshotPersistence<S> {
    async fn get(&self, id: &str) -> Result<Option<ValidState<S>>, BackendError> {
        if self.should_fail() {
            return Err(planned_failure());
        }
        self.inner.get(id).await
    }
    async fn put(&self, items: Vec<SnapshotItem<S>>) -> Result<(), BackendError> {
        if self.should_fail() {
            return Err(planned_failure());
        }
        self.inner.put(items).await
    }
}

// ---------------------------------------------------------------------------
// Event-sourcing repositories
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action<S, E, N> {
    Appended {
        cmd: CommandMessage<()>,
        version: SeqNr,
        new_state: S,
        events: NonEmpty<E>,
        notifications: Vec<N>,
    },
    Notified {
        cmd: CommandMessage<()>,
        notifications: NonEmpty<N>,
    },
}

pub struct FakeRepository<S, E, R, N> {
    state: CommandState<S, E, R>,
    actions: Mutex<Vec<Action<S, E, N>>>,
    loaded: Mutex<Vec<CommandMessage<()>>>,
    /// Number of `append` calls that must fail with a version conflict
    /// before one succeeds.
    conflicts: AtomicUsize,
}

impl<S, E, R, N> FakeRepository<S, E, R, N> {
    pub fn new(state: CommandState<S, E, R>) -> Self {
        Self {
            state,
            actions: Mutex::new(Vec::new()),
            loaded: Mutex::new(Vec::new()),
            conflicts: AtomicUsize::new(0),
        }
    }

    pub fn conflicting(state: CommandState<S, E, R>, conflicts: usize) -> Self {
        let repo = Self::new(state);
        repo.conflicts.store(conflicts, Ordering::SeqCst);
        repo
    }

    /// Recorded actions, oldest first.
    pub fn actions(&self) -> Vec<Action<S, E, N>>
    where
        S: Clone,
        E: Clone,
        N: Clone,
    {
        lock(&self.actions).clone()
    }

    /// Loaded commands, oldest first.
    pub fn loaded(&self) -> Vec<CommandMessage<()>> {
        lock(&self.loaded).clone()
    }
}

#[async_trait]
impl<S: Payload, E: Payload, R: Payload, N: Payload> Repository<S, E, R, N>
    for FakeRepository<S, E, R, N>
{
    async fn load(&self, cmd: CommandRef<'_>) -> Result<CommandState<S, E, R>, BackendError> {
        lock(&self.loaded).push(cmd.to_owned_message());
        Ok(self.state.clone())
    }

    async fn append(
        &self,
        cmd: CommandRef<'_>,
        version: SeqNr,
        new_state: S,
        events: NonEmpty<E>,
        notifications: Vec<N>,
    ) -> Result<(), BackendError> {
        if self.conflicts.load(Ordering::SeqCst) > 0 {
            self.conflicts.fetch_sub(1, Ordering::SeqCst);
            return Err(BackendError::VersionConflict);
        }
        lock(&self.actions).push(Action::Appended {
            cmd: cmd.to_owned_message(),
            version,
            new_state,
            events,
            notifications,
        });
        Ok(())
    }

    async fn notify(
        &self,
        cmd: CommandRef<'_>,
        notifications: NonEmpty<N>,
    ) -> Result<(), BackendError> {
        lock(&self.actions).push(Action::Notified {
            cmd: cmd.to_owned_message(),
            notifications,
        });
        Ok(())
    }
}

pub struct FailingRepository;

#[async_trait]
impl<S: Payload, E: Payload, R: Payload, N: Payload> Repository<S, E, R, N> for FailingRepository {
    async fn load(&self, _cmd: CommandRef<'_>) -> Result<CommandState<S, E, R>, BackendError> {
        Err(planned_failure())
    }
    async fn append(
        &self,
        _cmd: CommandRef<'_>,
        _version: SeqNr,
        _new_state: S,
        _events: NonEmpty<E>,
        _notifications: Vec<N>,
    ) -> Result<(), BackendError> {
        Err(planned_failure())
    }
    async fn notify(&self, _cmd: CommandRef<'_>, _ns: NonEmpty<N>) -> Result<(), BackendError> {
        Err(planned_failure())
    }
}

/// A repository whose `load` fails with the given error.
pub struct ErrorRepository(pub BackendError);

#[async_trait]
impl<S: Payload, E: Payload, R: Payload, N: Payload> Repository<S, E, R, N> for ErrorRepository {
    async fn load(&self, _cmd: CommandRef<'_>) -> Result<CommandState<S, E, R>, BackendError> {
        Err(clone_error(&self.0))
    }
    async fn append(
        &self,
        _cmd: CommandRef<'_>,
        _version: SeqNr,
        _new_state: S,
        _events: NonEmpty<E>,
        _notifications: Vec<N>,
    ) -> Result<(), BackendError> {
        Err(clone_error(&self.0))
    }
    async fn notify(&self, _cmd: CommandRef<'_>, _ns: NonEmpty<N>) -> Result<(), BackendError> {
        Err(clone_error(&self.0))
    }
}

pub fn clone_error(e: &BackendError) -> BackendError {
    match e {
        BackendError::VersionConflict => BackendError::VersionConflict,
        BackendError::MaxRetryExceeded => BackendError::MaxRetryExceeded,
        BackendError::PersistenceError(m) => BackendError::PersistenceError(m.clone()),
        BackendError::UnknownError(u) => BackendError::PersistenceError(u.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Journal stub
// ---------------------------------------------------------------------------

pub struct JournalReaderStub<E> {
    data: Vec<EventMessage<E>>,
}

impl<E> JournalReaderStub<E> {
    pub fn new(data: Vec<EventMessage<E>>) -> Self {
        Self { data }
    }
}

impl<E: Payload> JournalReaderStub<E> {
    fn select(
        &self,
        pred: impl Fn(&EventMessage<E>) -> bool,
    ) -> BoxStream<'_, Result<EventMessage<E>, BackendError>> {
        let rows: Vec<_> = self.data.iter().filter(|e| pred(e)).cloned().collect();
        Box::pin(futures::stream::iter(rows.into_iter().map(Ok)))
    }
}

impl<E: Payload> JournalReader<E> for JournalReaderStub<E> {
    fn read_stream(&self, stream_id: &str) -> BoxStream<'_, Result<EventMessage<E>, BackendError>> {
        let id = stream_id.to_owned();
        self.select(move |e| e.metadata.stream == id)
    }
    fn read_stream_after(
        &self,
        stream_id: &str,
        version: EventVersion,
    ) -> BoxStream<'_, Result<EventMessage<E>, BackendError>> {
        let id = stream_id.to_owned();
        self.select(move |e| e.metadata.stream == id && e.metadata.version > version)
    }
    fn read_stream_before(
        &self,
        stream_id: &str,
        version: EventVersion,
    ) -> BoxStream<'_, Result<EventMessage<E>, BackendError>> {
        let id = stream_id.to_owned();
        self.select(move |e| e.metadata.stream == id && e.metadata.version < version)
    }
    fn read_all(&self) -> BoxStream<'_, Result<EventMessage<E>, BackendError>> {
        self.select(|_| true)
    }
    fn read_all_after(
        &self,
        seq_nr: SeqNr,
    ) -> BoxStream<'_, Result<EventMessage<E>, BackendError>> {
        self.select(move |e| e.metadata.seq_nr > seq_nr)
    }
    fn read_all_before(
        &self,
        seq_nr: SeqNr,
    ) -> BoxStream<'_, Result<EventMessage<E>, BackendError>> {
        self.select(move |e| e.metadata.seq_nr < seq_nr)
    }
}

// ---------------------------------------------------------------------------
// Outbox reader
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marked<N>(pub NonEmpty<OutboxItem<N>>);

pub struct FakeOutboxReader<N> {
    data: Vec<OutboxItem<N>>,
    actions: Mutex<Vec<Marked<N>>>,
}

impl<N> FakeOutboxReader<N> {
    pub fn new(data: Vec<OutboxItem<N>>) -> Self {
        Self {
            data,
            actions: Mutex::new(Vec::new()),
        }
    }

    /// Marked batches, oldest first.
    pub fn list_actions(&self) -> Vec<Marked<N>>
    where
        N: Clone,
    {
        lock(&self.actions).clone()
    }
}

#[async_trait]
impl<N: Payload> OutboxReader<N> for FakeOutboxReader<N> {
    fn read(&self) -> BoxStream<'_, Result<OutboxItem<N>, BackendError>> {
        Box::pin(futures::stream::iter(self.data.clone().into_iter().map(Ok)))
    }
    async fn mark_all_as_sent(&self, items: &NonEmpty<OutboxItem<N>>) -> Result<(), BackendError> {
        lock(&self.actions).push(Marked(items.clone()));
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// CQRS fake repository
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Interaction<S, N> {
    Saved {
        cmd: CommandMessage<()>,
        version: SeqNr,
        new_state: S,
        events: Vec<N>,
    },
    Notified {
        cmd: CommandMessage<()>,
        events: NonEmpty<N>,
    },
}

pub struct CqrsFakeRepository<S, N> {
    state: cqrs::CommandState<S>,
    saved: Mutex<Vec<Interaction<S, N>>>,
    conflicts: AtomicUsize,
}

impl<S, N> CqrsFakeRepository<S, N> {
    pub fn new(state: cqrs::CommandState<S>) -> Self {
        Self {
            state,
            saved: Mutex::new(Vec::new()),
            conflicts: AtomicUsize::new(0),
        }
    }

    pub fn conflicting(state: cqrs::CommandState<S>, conflicts: usize) -> Self {
        let repo = Self::new(state);
        repo.conflicts.store(conflicts, Ordering::SeqCst);
        repo
    }

    /// Interactions, oldest first.
    pub fn saved(&self) -> Vec<Interaction<S, N>>
    where
        S: Clone,
        N: Clone,
    {
        lock(&self.saved).clone()
    }
}

#[async_trait]
impl<S: Payload, N: Payload> cqrs::RepositoryReader<S> for CqrsFakeRepository<S, N> {
    async fn get(&self, _id: &str) -> Result<cqrs::AggregateState<S>, BackendError> {
        match &self.state {
            cqrs::CommandState::Aggregate(a) => Ok(a.clone()),
            cqrs::CommandState::Redundant => {
                Err(BackendError::persistence("don't know any state!"))
            }
        }
    }
}

#[async_trait]
impl<S: Payload, N: Payload> cqrs::Repository<S, N> for CqrsFakeRepository<S, N> {
    async fn load(&self, _cmd: CommandRef<'_>) -> Result<cqrs::CommandState<S>, BackendError> {
        Ok(self.state.clone())
    }

    async fn save(
        &self,
        cmd: CommandRef<'_>,
        version: SeqNr,
        new_state: S,
        events: Vec<N>,
    ) -> Result<(), BackendError> {
        if self.conflicts.load(Ordering::SeqCst) > 0 {
            self.conflicts.fetch_sub(1, Ordering::SeqCst);
            return Err(BackendError::VersionConflict);
        }
        lock(&self.saved).push(Interaction::Saved {
            cmd: cmd.to_owned_message(),
            version,
            new_state,
            events,
        });
        Ok(())
    }

    async fn notify(&self, cmd: CommandRef<'_>, events: NonEmpty<N>) -> Result<(), BackendError> {
        lock(&self.saved).push(Interaction::Notified {
            cmd: cmd.to_owned_message(),
            events,
        });
        Ok(())
    }
}

pub struct CqrsFailingRepository;

#[async_trait]
impl<S: Payload> cqrs::RepositoryReader<S> for CqrsFailingRepository {
    async fn get(&self, _id: &str) -> Result<cqrs::AggregateState<S>, BackendError> {
        Err(planned_failure())
    }
}

#[async_trait]
impl<S: Payload, N: Payload> cqrs::Repository<S, N> for CqrsFailingRepository {
    async fn load(&self, _cmd: CommandRef<'_>) -> Result<cqrs::CommandState<S>, BackendError> {
        Err(planned_failure())
    }
    async fn save(
        &self,
        _cmd: CommandRef<'_>,
        _version: SeqNr,
        _new_state: S,
        _events: Vec<N>,
    ) -> Result<(), BackendError> {
        Err(planned_failure())
    }
    async fn notify(&self, _cmd: CommandRef<'_>, _events: NonEmpty<N>) -> Result<(), BackendError> {
        Err(planned_failure())
    }
}

/// Erases the payload of a command message (what fakes record).
pub fn erased<C>(cmd: &CommandMessage<C>) -> CommandMessage<()> {
    CommandRef::from(cmd).to_owned_message()
}

pub fn valid<S, E, R>(state: S, version: SeqNr) -> CommandState<S, E, R> {
    CommandState::Aggregate(AggregateState::valid(state, version))
}
