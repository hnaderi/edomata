//! Port of `OutboxConsumerSuite.scala`.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use chrono::{DateTime, Duration, Utc};
use common::*;
use edomata_backend::{BackendError, OutboxConsumer, OutboxItem};
use edomata_core::MessageMetadata;

fn items_for(values: impl IntoIterator<Item = i32>) -> Vec<OutboxItem<i32>> {
    values
        .into_iter()
        .enumerate()
        .map(|(id, i)| OutboxItem {
            seq_nr: id as i64,
            stream_id: "sut".into(),
            time: DateTime::<Utc>::MIN_UTC + Duration::days(id as i64),
            data: i,
            metadata: MessageMetadata::root(id.to_string()),
        })
        .collect()
}

#[tokio::test]
async fn empty() {
    let fo = FakeOutboxReader::<i32>::new(vec![]);
    OutboxConsumer::new()
        .run(&fo, futures::stream::empty(), |_| async {
            panic!("How in the world?")
        })
        .await
        .unwrap();
    assert!(fo.list_actions().is_empty());
}

#[tokio::test]
async fn must_run_action_on_all_consumed_items() {
    let fo = FakeOutboxReader::new(items_for(10..20));
    let counter = Arc::new(AtomicI64::new(0));
    let c = Arc::clone(&counter);
    OutboxConsumer::new()
        .run(&fo, futures::stream::empty(), move |item| {
            let c = Arc::clone(&c);
            async move {
                assert_eq!(c.fetch_add(1, Ordering::SeqCst), item.seq_nr);
                assert_eq!(item.data as i64, item.seq_nr + 10);
                assert_eq!(
                    item.time,
                    DateTime::<Utc>::MIN_UTC + Duration::days(item.seq_nr)
                );
                assert_eq!(item.stream_id, "sut");
                Ok(())
            }
        })
        .await
        .unwrap();
    assert_eq!(counter.load(Ordering::SeqCst), 10);
}

#[tokio::test]
async fn must_mark_each_chunk_as_read_after_successful_run() {
    // 1, 2..5, 6..20 as in the Scala suite; chunks are batches of 4 items.
    let data = items_for(std::iter::once(1).chain(2..5).chain(6..20));
    let fo = FakeOutboxReader::new(data);
    OutboxConsumer::new()
        .with_batch_size(4)
        .run(&fo, futures::stream::empty(), |_| async { Ok(()) })
        .await
        .unwrap();
    let marked: Vec<Vec<i32>> = fo
        .list_actions()
        .into_iter()
        .map(|m| m.0.iter().map(|i| i.data).collect())
        .collect();
    assert_eq!(
        marked,
        vec![
            vec![1, 2, 3, 4],
            vec![6, 7, 8, 9],
            vec![10, 11, 12, 13],
            vec![14, 15, 16, 17],
            vec![18, 19],
        ]
    );
}

#[tokio::test]
async fn must_not_mark_when_the_handler_fails() {
    let fo = FakeOutboxReader::new(items_for(1..4));
    let err = OutboxConsumer::new()
        .run(&fo, futures::stream::empty(), |item| async move {
            if item.data == 2 {
                Err(BackendError::persistence("boom"))
            } else {
                Ok(())
            }
        })
        .await;
    assert_eq!(err, Err(BackendError::persistence("boom")));
    assert!(fo.list_actions().is_empty());
}

#[tokio::test]
async fn runs_again_on_each_signal() {
    let fo = FakeOutboxReader::new(items_for(1..3));
    let counter = Arc::new(AtomicI64::new(0));
    let c = Arc::clone(&counter);
    OutboxConsumer::new()
        .run(&fo, futures::stream::iter([(), ()]), move |_| {
            let c = Arc::clone(&c);
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        })
        .await
        .unwrap();
    // once immediately + once per signal = 3 passes over 2 items
    assert_eq!(counter.load(Ordering::SeqCst), 6);
    assert_eq!(fo.list_actions().len(), 3);
}
