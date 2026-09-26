//! Port of `CqrsSuite.scala`.

use std::time::Duration;

use chrono::DateTime;
use edomata_backend::BackendError;
use edomata_backend::cqrs::{AggregateState, Backend};
use edomata_core::CommandMessage;
use futures::{StreamExt, TryStreamExt};

use crate::{random_string, test_cqrs_dsl};

/// The CQRS backend type the suite runs against.
pub type CqrsBackend = Backend<i32, i32>;

fn some_cmd() -> CommandMessage<i32> {
    CommandMessage::new(random_string(), DateTime::UNIX_EPOCH, random_string(), 0)
}

async fn assert_notified(mut stream: futures::stream::BoxStream<'static, ()>, what: &str) {
    tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .unwrap_or_else(|_| panic!("{what} listeners were not notified"))
        .unwrap_or_else(|| panic!("{what} signal ended"));
}

async fn assert_notified_state(b: &CqrsBackend) {
    assert_notified(b.updates().state(), "state").await;
}

async fn assert_notified_outbox(b: &CqrsBackend) {
    assert_notified(b.updates().outbox(), "outbox").await;
}

/// "inserts state"
pub async fn inserts_state(b: &CqrsBackend) {
    let dsl = test_cqrs_dsl();
    let srv = b.compile(dsl.set(2));
    let agg_id = random_string();
    let cmd_id = random_string();
    assert_eq!(
        srv(CommandMessage::new(
            cmd_id,
            DateTime::UNIX_EPOCH,
            agg_id.clone(),
            0
        ))
        .await
        .unwrap(),
        Ok(())
    );
    assert_eq!(
        b.repository().get(&agg_id).await.unwrap(),
        AggregateState::new(2, 1)
    );
    assert_notified_state(b).await;
}

/// "updates existing state"
pub async fn updates_existing_state(b: &CqrsBackend) {
    let dsl = test_cqrs_dsl();
    let srv = b.compile(dsl.router(move |i| dsl.set(i)));
    let agg_id = random_string();
    srv(CommandMessage::new(
        random_string(),
        DateTime::UNIX_EPOCH,
        agg_id.clone(),
        2,
    ))
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        srv(CommandMessage::new(
            random_string(),
            DateTime::UNIX_EPOCH,
            agg_id.clone(),
            5
        ))
        .await
        .unwrap(),
        Ok(())
    );
    assert_eq!(
        b.repository().get(&agg_id).await.unwrap(),
        AggregateState::new(5, 2)
    );
    assert_notified_state(b).await;
}

/// "publishes notifications"
pub async fn publishes_notifications(b: &CqrsBackend) {
    let dsl = test_cqrs_dsl();
    let srv = b.compile(dsl.publish([1, 2, 3]));
    let agg_id = random_string();
    assert_eq!(
        srv(CommandMessage::new(
            random_string(),
            DateTime::UNIX_EPOCH,
            agg_id.clone(),
            0
        ))
        .await
        .unwrap(),
        Ok(())
    );
    assert_eq!(
        b.repository().get(&agg_id).await.unwrap(),
        AggregateState::new(0, 1)
    );
    let published: Vec<i32> = b
        .outbox()
        .read()
        .try_filter(|i| std::future::ready(i.stream_id == agg_id))
        .map_ok(|i| i.data)
        .try_collect()
        .await
        .unwrap();
    assert_eq!(published, vec![1, 2, 3]);
    assert_notified_outbox(b).await;
}

/// "save must be idempotent"
pub async fn save_must_be_idempotent(s: &CqrsBackend) {
    let dsl = test_cqrs_dsl();
    let cmd = some_cmd();
    let srv = s.compile(dsl.modify(|s| s + 10).publish([4, 5, 6]).void());
    let attempts = futures::future::join_all((0..20).map(|_| srv(cmd.clone()))).await;
    for outcome in attempts {
        match outcome {
            Ok(_) | Err(BackendError::VersionConflict) => {}
            Err(other) => panic!("Invalid implementation: {other}"),
        }
    }
    assert_eq!(
        s.repository().get(&cmd.address).await.unwrap(),
        AggregateState::new(10, 1)
    );
    assert_notified_state(s).await;
    assert_notified_outbox(s).await;
}

/// "save must be correct"
pub async fn save_must_be_correct(s: &CqrsBackend) {
    let dsl = test_cqrs_dsl();
    let agg_id = random_string();
    let srv = s.compile(dsl.modify(|s| s + 10).publish([4, 5, 6]).void());
    let attempts = futures::future::join_all((0..5).map(|_| {
        let mut cmd = some_cmd();
        cmd.address = agg_id.clone();
        srv(cmd)
    }))
    .await;
    for outcome in attempts {
        match outcome {
            Ok(_) => {}
            Err(BackendError::VersionConflict) => panic!("bad luck!"),
            Err(other) => panic!("Invalid implementation: {other}"),
        }
    }
    assert_eq!(
        s.repository().get(&agg_id).await.unwrap(),
        AggregateState::new(50, 5)
    );
    assert_notified_state(s).await;
    assert_notified_outbox(s).await;
}
