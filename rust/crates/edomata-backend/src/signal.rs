//! In-process update signals.

use std::sync::Arc;

use futures::Stream;
use futures::stream::BoxStream;
use tokio::sync::Notify;

/// A coalescing, in-process signal: the counterpart of Scala's
/// `Queue.circularBuffer[F, Unit](1)`.
///
/// Notifying stores at most one pending signal, so a burst of notifications
/// wakes the listener once; a signal raised before anyone listens is kept
/// until it is consumed.
#[derive(Clone, Debug, Default)]
pub struct Signal {
    notify: Arc<Notify>,
}

impl Signal {
    /// Creates a signal.
    pub fn new() -> Self {
        Self::default()
    }

    /// Raises the signal.
    pub fn notify(&self) {
        self.notify.notify_one();
    }

    /// Waits for the next signal.
    pub async fn wait(&self) {
        self.notify.notified().await;
    }

    /// An endless stream that yields once per (coalesced) signal.
    pub fn stream(&self) -> impl Stream<Item = ()> + Send + 'static {
        futures::stream::unfold(Arc::clone(&self.notify), |notify| async move {
            notify.notified().await;
            Some(((), notify))
        })
    }

    /// [`Signal::stream`], boxed.
    pub fn boxed_stream(&self) -> BoxStream<'static, ()> {
        Box::pin(self.stream())
    }
}
