//! Port of `eventsourcing/RepositoryReaderSuite.scala`.

mod common;

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use common::*;
use edomata_backend::eventsourcing::{
    AggregateState, JournalReader, JournalRepositoryReader, RepositoryReader, SnapshotReader,
    ValidState,
};
use edomata_backend::{EventMessage, EventMetadata, SharedModel};
use edomata_core::{DomainModel, NonEmpty};
use futures::TryStreamExt;
use uuid::Uuid;

struct SutDomain;

impl DomainModel for SutDomain {
    type State = i64;
    type Event = i32;
    type Rejection = String;

    fn initial(&self) -> i64 {
        0
    }

    fn transition(&self, i: &i32, l: i64) -> Result<i64, NonEmpty<String>> {
        Ok(*i as i64 + l)
    }
}

fn model() -> SharedModel<i64, i32, String> {
    Arc::new(SutDomain)
}

fn gen_list(stream_id: &str, n: i64) -> Vec<EventMessage<i32>> {
    (0..n)
        .map(|i| EventMessage {
            metadata: EventMetadata {
                id: Uuid::from_u64_pair(i as u64, i as u64),
                time: DateTime::<Utc>::MIN_UTC + Duration::minutes(i),
                seq_nr: 10 * i,
                version: i,
                stream: stream_id.to_owned(),
            },
            payload: i as i32 + 1,
        })
        .collect()
}

fn initial() -> AggregateState<i64, i32, String> {
    AggregateState::Valid(ValidState::new(0, 0))
}

fn data() -> Vec<EventMessage<i32>> {
    gen_list("sut", 10)
}

fn expected_history() -> Vec<AggregateState<i64, i32, String>> {
    let mut out = vec![initial()];
    let mut state = ValidState::new(0i64, 0);
    for e in data() {
        state = ValidState::new(state.state + e.payload as i64, state.version + 1);
        out.push(AggregateState::Valid(state.clone()));
    }
    out
}

fn reader(snapshot: Arc<dyn SnapshotReader<i64>>) -> JournalRepositoryReader<i64, i32, String> {
    let journal: Arc<dyn JournalReader<i32>> = Arc::new(JournalReaderStub::new(data()));
    JournalRepositoryReader::new(journal, snapshot, model())
}

async fn history(
    r: &JournalRepositoryReader<i64, i32, String>,
) -> Vec<AggregateState<i64, i32, String>> {
    r.history("sut").try_collect().await.unwrap()
}

#[tokio::test]
async fn sanity() {
    let r = reader(Arc::new(BlackHoleSnapshotStore));
    assert_eq!(r.get("sut").await.unwrap(), AggregateState::valid(55, 10));
    assert_eq!(history(&r).await, expected_history());
}

#[tokio::test]
async fn repository_reader_uses_snapshot() {
    let r = reader(Arc::new(ConstantSnapshotStore {
        state: 45i64,
        version: 9,
    }));
    assert_eq!(r.get("sut").await.unwrap(), AggregateState::valid(55, 10));
    assert_eq!(history(&r).await, expected_history());
}

#[tokio::test]
async fn repository_reader_uses_wrong_snapshots_too() {
    let r = reader(Arc::new(ConstantSnapshotStore {
        state: 100i64,
        version: 9,
    }));
    assert_eq!(r.get("sut").await.unwrap(), AggregateState::valid(110, 10));
    assert_eq!(history(&r).await, expected_history());
}

#[tokio::test]
async fn unknown_stream_is_the_initial_state() {
    let r = reader(Arc::new(BlackHoleSnapshotStore));
    assert_eq!(r.get("unknown").await.unwrap(), initial());
    assert_eq!(history(&r).await.len(), 11);
}

#[tokio::test]
async fn history_stops_at_the_first_conflict() {
    struct Picky;
    impl DomainModel for Picky {
        type State = i64;
        type Event = i32;
        type Rejection = String;
        fn initial(&self) -> i64 {
            0
        }
        fn transition(&self, i: &i32, l: i64) -> Result<i64, NonEmpty<String>> {
            if *i == 3 {
                Err(NonEmpty::new("three".into()))
            } else {
                Ok(*i as i64 + l)
            }
        }
    }
    let journal: Arc<dyn JournalReader<i32>> = Arc::new(JournalReaderStub::new(data()));
    let r =
        JournalRepositoryReader::new(journal, Arc::new(BlackHoleSnapshotStore), Arc::new(Picky));
    let h = history(&r).await;
    assert_eq!(h.len(), 4); // initial, +1, +2, conflict on 3
    assert!(matches!(
        h.last(),
        Some(AggregateState::Conflicted { last: 3, .. })
    ));
    assert!(!r.get("sut").await.unwrap().is_valid());
}
