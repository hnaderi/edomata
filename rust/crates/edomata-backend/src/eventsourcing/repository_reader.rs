//! Folding the journal into aggregate states.

use std::sync::Arc;

use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::{Stream, StreamExt, TryStreamExt};

use super::{AggregateState, JournalReader, SnapshotReader, ValidState};
use crate::{BackendError, EventMessage, Payload, SharedModel};

/// Reads the current state and the history of an aggregate.
#[async_trait]
pub trait RepositoryReader<S, E, R>: Send + Sync {
    /// Current state of the aggregate, folded from its snapshot and journal.
    async fn get(&self, stream_id: &str) -> Result<AggregateState<S, E, R>, BackendError>;

    /// Every state the aggregate went through, from the initial state to the
    /// current one (or to the first conflict, included).
    fn history(
        &self,
        stream_id: &str,
    ) -> BoxStream<'_, Result<AggregateState<S, E, R>, BackendError>>;
}

/// The default [`RepositoryReader`]: folds the journal with the model,
/// starting from the latest snapshot when there is one.
pub struct JournalRepositoryReader<S, E, R> {
    journal: Arc<dyn JournalReader<E>>,
    snapshot: Arc<dyn SnapshotReader<S>>,
    model: SharedModel<S, E, R>,
}

impl<S, E, R> JournalRepositoryReader<S, E, R> {
    /// Builds a reader over a journal and a snapshot reader.
    pub fn new(
        journal: Arc<dyn JournalReader<E>>,
        snapshot: Arc<dyn SnapshotReader<S>>,
        model: SharedModel<S, E, R>,
    ) -> Self {
        Self {
            journal,
            snapshot,
            model,
        }
    }
}

#[async_trait]
impl<S: Payload, E: Payload, R: Payload> RepositoryReader<S, E, R>
    for JournalRepositoryReader<S, E, R>
{
    async fn get(&self, stream_id: &str) -> Result<AggregateState<S, E, R>, BackendError> {
        match self.snapshot.get(stream_id).await? {
            Some(last) => {
                let events = self.journal.read_stream_after(stream_id, last.version - 1);
                last_state(scan_state(
                    events,
                    AggregateState::Valid(last),
                    &*self.model,
                ))
                .await
            }
            None => last_state(self.history(stream_id)).await,
        }
    }

    fn history(
        &self,
        stream_id: &str,
    ) -> BoxStream<'_, Result<AggregateState<S, E, R>, BackendError>> {
        let initial = AggregateState::Valid(ValidState::new(self.model.initial(), 0));
        Box::pin(scan_state(
            self.journal.read_stream(stream_id),
            initial,
            &*self.model,
        ))
    }
}

/// Emits `last`, then every state obtained by applying the events in order,
/// stopping after the first conflict (which is emitted).
pub(crate) fn scan_state<'a, S, E, R>(
    events: impl Stream<Item = Result<EventMessage<E>, BackendError>> + Send + 'a,
    last: AggregateState<S, E, R>,
    model: &'a (dyn edomata_core::DomainModel<State = S, Event = E, Rejection = R> + Send + Sync),
) -> impl Stream<Item = Result<AggregateState<S, E, R>, BackendError>> + Send + 'a
where
    S: Payload,
    E: Payload,
    R: Payload,
{
    let folded = events.scan(Some(last.clone()), move |current, event| {
        let next = match (current.take(), event) {
            (None, _) => None,
            (Some(_), Err(e)) => Some(Err(e)),
            (Some(AggregateState::Valid(ValidState { state, version })), Ok(ev)) => {
                match model.transition(&ev.payload, state.clone()) {
                    Ok(s) => {
                        let next = AggregateState::valid(s, version + 1);
                        *current = Some(next.clone());
                        Some(Ok(next))
                    }
                    Err(errors) => Some(Ok(AggregateState::Conflicted {
                        last: state,
                        on_event: ev,
                        errors,
                    })),
                }
            }
            (Some(conflicted @ AggregateState::Conflicted { .. }), Ok(_)) => {
                // Scala keeps emitting the conflicted state but `takeWhile`
                // stops right after it; we stop as well.
                let _ = conflicted;
                None
            }
        };
        std::future::ready(next)
    });
    futures::stream::once(std::future::ready(Ok(last))).chain(folded)
}

async fn last_state<S, E, R>(
    stream: impl Stream<Item = Result<AggregateState<S, E, R>, BackendError>> + Send,
) -> Result<AggregateState<S, E, R>, BackendError> {
    let mut stream = std::pin::pin!(stream);
    let mut last = None;
    while let Some(item) = stream.try_next().await? {
        last = Some(item);
    }
    last.ok_or_else(|| BackendError::persistence("empty aggregate history"))
}
