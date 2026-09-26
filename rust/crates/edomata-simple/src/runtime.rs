//! A blocking entry point over a Tokio runtime, the counterpart of
//! `EdomataRuntime`.

use std::future::Future;
use std::sync::Arc;

use edomata_backend::{EventMessage, OutboxItem, Payload};
use edomata_core::CommandMessage;
use tokio::runtime::{Handle, Runtime};

use crate::backend::HandleResult;
use crate::{CommandHandler, SimpleBackend, SimpleError};

/// The runtime blocking calls run on, the counterpart of Scala's
/// `EdomataRuntime`. One deliberate difference: Scala's `create()` wraps
/// the global `IORuntime` and owns nothing, but Tokio has no global
/// runtime, so [`SimpleRuntime::create`] owns a multi-threaded runtime
/// that [`close`](SimpleRuntime::close) shuts down.
/// [`SimpleRuntime::from_handle`] borrows an existing runtime (Scala's
/// `fromExisting`), which `close` never shuts down.
#[derive(Clone, Debug)]
pub struct SimpleRuntime {
    owned: Option<Arc<Runtime>>,
    handle: Handle,
}

impl SimpleRuntime {
    /// A new, owned runtime with default settings (Scala's
    /// `EdomataRuntime.create`, except that the runtime is owned, see the
    /// type documentation). Must not be called from within an asynchronous
    /// context.
    pub fn create() -> std::io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        Ok(Self {
            handle: runtime.handle().clone(),
            owned: Some(Arc::new(runtime)),
        })
    }

    /// Wraps an existing runtime handle (`EdomataRuntime.fromExisting`);
    /// the caller keeps ownership.
    pub fn from_handle(handle: Handle) -> Self {
        Self {
            owned: None,
            handle,
        }
    }

    /// The Tokio handle.
    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    /// Whether this value owns its runtime.
    pub fn owns_runtime(&self) -> bool {
        self.owned.is_some()
    }

    /// Runs a future to completion on this runtime.
    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.handle.block_on(future)
    }

    /// Shuts the runtime down when owned (`EdomataRuntime.close`). Pending
    /// tasks are cancelled; clones sharing the runtime keep it alive until
    /// they are dropped.
    pub fn close(self) {
        if let Some(runtime) = self.owned
            && let Ok(runtime) = Arc::try_unwrap(runtime)
        {
            runtime.shutdown_background();
        }
    }
}

/// A [`SimpleBackend`] with blocking methods, for callers outside an
/// asynchronous context (the `CompletableFuture`-free view of `JBackend`).
pub struct BlockingBackend<S, E, R, N> {
    backend: SimpleBackend<S, E, R, N>,
    runtime: SimpleRuntime,
}

impl<S, E, R, N> std::fmt::Debug for BlockingBackend<S, E, R, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BlockingBackend")
    }
}

impl<S: Payload, E: Payload, R: Payload, N: Payload> BlockingBackend<S, E, R, N> {
    /// Wraps a backend with a runtime.
    pub fn new(backend: SimpleBackend<S, E, R, N>, runtime: SimpleRuntime) -> Self {
        Self { backend, runtime }
    }

    /// The asynchronous backend.
    pub fn backend(&self) -> &SimpleBackend<S, E, R, N> {
        &self.backend
    }

    /// The runtime.
    pub fn runtime(&self) -> &SimpleRuntime {
        &self.runtime
    }

    /// Handles one command, blocking until done.
    pub fn handle<C: Payload>(
        &self,
        handler: &CommandHandler<C, S, E, R, N>,
        command: CommandMessage<C>,
    ) -> HandleResult<R> {
        self.runtime.block_on(self.backend.handle(handler, command))
    }

    /// All events of a stream.
    pub fn read_stream(&self, stream_id: &str) -> Result<Vec<EventMessage<E>>, SimpleError> {
        self.runtime
            .block_on(self.backend.journal().read_stream(stream_id))
    }

    /// The events of a stream after a version.
    pub fn read_stream_after(
        &self,
        stream_id: &str,
        version: i64,
    ) -> Result<Vec<EventMessage<E>>, SimpleError> {
        self.runtime
            .block_on(self.backend.journal().read_stream_after(stream_id, version))
    }

    /// All events across all streams.
    pub fn read_all(&self) -> Result<Vec<EventMessage<E>>, SimpleError> {
        self.runtime.block_on(self.backend.journal().read_all())
    }

    /// All events across all streams after a sequence number.
    pub fn read_all_after(&self, seq_nr: i64) -> Result<Vec<EventMessage<E>>, SimpleError> {
        self.runtime
            .block_on(self.backend.journal().read_all_after(seq_nr))
    }

    /// All pending outbox items.
    pub fn read_outbox(&self) -> Result<Vec<OutboxItem<N>>, SimpleError> {
        self.runtime.block_on(self.backend.outbox().read())
    }

    /// Marks one outbox item as sent.
    pub fn mark_as_sent(&self, item: &OutboxItem<N>) -> Result<(), SimpleError> {
        self.runtime
            .block_on(self.backend.outbox().mark_as_sent(item))
    }

    /// Marks outbox items as sent.
    pub fn mark_all_as_sent(&self, items: &[OutboxItem<N>]) -> Result<(), SimpleError> {
        self.runtime
            .block_on(self.backend.outbox().mark_all_as_sent(items))
    }

    /// Releases the backend's resources, blocking until done. The runtime
    /// is left to the caller.
    pub fn close(&self) -> Result<(), SimpleError> {
        self.runtime.block_on(self.backend.close())
    }
}
