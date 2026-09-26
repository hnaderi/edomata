//! In-process update signals for CQRS backends.

use futures::stream::BoxStream;

use crate::Signal;

/// Listens to storage updates.
pub trait NotificationsConsumer: Send + Sync {
    /// Yields whenever new outbox items were written.
    fn outbox(&self) -> BoxStream<'static, ()>;
    /// Yields whenever a state was saved.
    fn state(&self) -> BoxStream<'static, ()>;
}

/// Raises storage update signals.
pub trait NotificationsPublisher: Send + Sync {
    /// Signals new outbox items.
    fn notify_outbox(&self);
    /// Signals a saved state.
    fn notify_state(&self);
}

/// Coalescing in-process signals for the outbox and the states.
#[derive(Clone, Debug, Default)]
pub struct Notifications {
    outbox: Signal,
    state: Signal,
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

    fn state(&self) -> BoxStream<'static, ()> {
        self.state.boxed_stream()
    }
}

impl NotificationsPublisher for Notifications {
    fn notify_outbox(&self) {
        self.outbox.notify();
    }

    fn notify_state(&self) {
        self.state.notify();
    }
}
