//! Reading the journal.

use futures::stream::BoxStream;

use crate::{BackendError, EventMessage, EventVersion, SeqNr};

/// Reads journaled events. Every stream is ordered: by version for a single
/// aggregate, by global sequence number for the whole journal.
pub trait JournalReader<E>: Send + Sync {
    /// All events of one aggregate, by ascending version.
    fn read_stream(&self, stream_id: &str) -> BoxStream<'_, Result<EventMessage<E>, BackendError>>;

    /// Events of one aggregate with a version strictly greater than
    /// `version`.
    fn read_stream_after(
        &self,
        stream_id: &str,
        version: EventVersion,
    ) -> BoxStream<'_, Result<EventMessage<E>, BackendError>>;

    /// Events of one aggregate with a version strictly lower than `version`.
    fn read_stream_before(
        &self,
        stream_id: &str,
        version: EventVersion,
    ) -> BoxStream<'_, Result<EventMessage<E>, BackendError>>;

    /// The whole journal, by ascending sequence number.
    fn read_all(&self) -> BoxStream<'_, Result<EventMessage<E>, BackendError>>;

    /// Events with a sequence number strictly greater than `seq_nr`.
    fn read_all_after(&self, seq_nr: SeqNr)
    -> BoxStream<'_, Result<EventMessage<E>, BackendError>>;

    /// Events with a sequence number strictly lower than `seq_nr`.
    fn read_all_before(
        &self,
        seq_nr: SeqNr,
    ) -> BoxStream<'_, Result<EventMessage<E>, BackendError>>;
}
