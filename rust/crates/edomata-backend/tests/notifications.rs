//! Port of `eventsourcing/NotificationsSuite.scala` and
//! `cqrs/NotificationsSuite.scala`.

use std::time::Duration;

use edomata_backend::{cqrs, eventsourcing};
use futures::StreamExt;
use futures::stream::BoxStream;

/// Asserts that the stream yields exactly one signal within 10 hours of
/// (virtual) time: the first `next` completes, the second one times out.
async fn assert_notified_once(mut s: BoxStream<'static, ()>) {
    assert_eq!(s.next().await, Some(()));
    let second = tokio::time::timeout(Duration::from_secs(10 * 3600), s.next()).await;
    assert!(second.is_err(), "notified more than once");
}

#[tokio::test(start_paused = true)]
async fn es_must_notify_outbox_listeners() {
    use eventsourcing::{NotificationsConsumer, NotificationsPublisher};
    let ns = eventsourcing::Notifications::new();
    ns.notify_outbox();
    assert_notified_once(ns.outbox()).await;
}

#[tokio::test(start_paused = true)]
async fn es_must_notify_outbox_listeners_once() {
    use eventsourcing::{NotificationsConsumer, NotificationsPublisher};
    let ns = eventsourcing::Notifications::new();
    ns.notify_outbox();
    ns.notify_outbox();
    ns.notify_outbox();
    assert_notified_once(ns.outbox()).await;
}

#[tokio::test(start_paused = true)]
async fn es_must_notify_journal_listeners() {
    use eventsourcing::{NotificationsConsumer, NotificationsPublisher};
    let ns = eventsourcing::Notifications::new();
    ns.notify_journal();
    assert_notified_once(ns.journal()).await;
}

#[tokio::test(start_paused = true)]
async fn es_must_notify_journal_listeners_once() {
    use eventsourcing::{NotificationsConsumer, NotificationsPublisher};
    let ns = eventsourcing::Notifications::new();
    ns.notify_journal();
    ns.notify_journal();
    ns.notify_journal();
    assert_notified_once(ns.journal()).await;
}

#[tokio::test(start_paused = true)]
async fn cqrs_must_notify_outbox_listeners() {
    use cqrs::{NotificationsConsumer, NotificationsPublisher};
    let ns = cqrs::Notifications::new();
    ns.notify_outbox();
    assert_notified_once(ns.outbox()).await;
}

#[tokio::test(start_paused = true)]
async fn cqrs_must_notify_outbox_listeners_once() {
    use cqrs::{NotificationsConsumer, NotificationsPublisher};
    let ns = cqrs::Notifications::new();
    ns.notify_outbox();
    ns.notify_outbox();
    ns.notify_outbox();
    assert_notified_once(ns.outbox()).await;
}

#[tokio::test(start_paused = true)]
async fn cqrs_must_notify_state_listeners() {
    use cqrs::{NotificationsConsumer, NotificationsPublisher};
    let ns = cqrs::Notifications::new();
    ns.notify_state();
    assert_notified_once(ns.state()).await;
}

#[tokio::test(start_paused = true)]
async fn cqrs_must_notify_state_listeners_once() {
    use cqrs::{NotificationsConsumer, NotificationsPublisher};
    let ns = cqrs::Notifications::new();
    ns.notify_state();
    ns.notify_state();
    ns.notify_state();
    assert_notified_once(ns.state()).await;
}

#[tokio::test(start_paused = true)]
async fn listeners_are_independent() {
    use eventsourcing::{NotificationsConsumer, NotificationsPublisher};
    let ns = eventsourcing::Notifications::new();
    ns.notify_journal();
    let outbox = tokio::time::timeout(Duration::from_secs(3600), ns.outbox().next()).await;
    assert!(outbox.is_err(), "outbox must not be notified by journal");
    assert_notified_once(ns.journal()).await;
}
