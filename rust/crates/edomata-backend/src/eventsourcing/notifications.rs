//! In-process update signals for event-sourced backends.

use futures::stream::BoxStream;

use crate::Signal;

/// Listens to storage updates.
pub trait NotificationsConsumer: Send + Sync {
    /// Yields whenever new outbox items were written.
    fn outbox(&self) -> BoxStream<'static, ()>;
    /// Yields whenever new events were journaled.
    fn journal(&self) -> BoxStream<'static, ()>;
}

/// Raises storage update signals.
pub trait NotificationsPublisher: Send + Sync {
    /// Signals new outbox items.
    fn notify_outbox(&self);
    /// Signals new journal events.
    fn notify_journal(&self);
}

/// Coalescing in-process signals for the outbox and the journal. Mirrors
/// Scala's `Notifications[F]` built on `Queue.circularBuffer(1)`.
#[derive(Clone, Debug, Default)]
pub struct Notifications {
    outbox: Signal,
    journal: Signal,
}

impl Notifications {
    /// Creates a new pair of signals.
    pub fn new() -> Self {
        Self::default()
    }
}

impl NotificationsConsumer for Notifications {
    fn outbox(&self) -> BoxStream<'static, ()> {
        self.outbox.boxed_stream()
    }

    fn journal(&self) -> BoxStream<'static, ()> {
        self.journal.boxed_stream()
    }
}

impl NotificationsPublisher for Notifications {
    fn notify_outbox(&self) {
        self.outbox.notify();
    }

    fn notify_journal(&self) {
        self.journal.notify();
    }
}
