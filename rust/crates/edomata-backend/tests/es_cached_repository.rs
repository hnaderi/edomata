//! Port of `eventsourcing/CachedRepositorySuite.scala`.

mod common;

use std::collections::HashMap;
use std::sync::Arc;

use common::*;
use edomata_backend::eventsourcing::{CachedRepository, CommandState, Repository, ValidState};
use edomata_backend::{CommandRef, SeqNr};
use edomata_core::{CommandMessage, NonEmpty, nonempty};

type Repo = FakeRepository<i32, i32, String, i32>;
type Cached = CachedRepository<i32, i32, String, i32>;

fn some_cmd() -> CommandMessage<i32> {
    CommandMessage::new("", max_time(), "sut", 1)
}

fn persisted_state() -> CommandState<i32, i32, String> {
    valid(1, 0)
}

fn in_mem_state() -> CommandState<i32, i32, String> {
    valid(10, 3)
}

#[tokio::test]
async fn must_fallback_to_underlying_repository_when_cant_optimize() {
    let repo = Arc::new(Repo::new(persisted_state()));
    let cr = Cached::new(
        repo.clone(),
        Arc::new(BlackHoleCommandStore),
        Arc::new(BlackHoleSnapshotStore),
    );
    let cmd = some_cmd();
    assert_eq!(
        cr.load(CommandRef::from(&cmd)).await.unwrap(),
        persisted_state()
    );
    assert_eq!(repo.loaded(), vec![erased(&cmd)]);
}

#[tokio::test]
async fn must_short_circuit_when_knows_its_a_redundant_command() {
    let repo = Arc::new(Repo::new(persisted_state()));
    let cr = Cached::new(
        repo.clone(),
        Arc::new(YesManCommandStore),
        Arc::new(BlackHoleSnapshotStore),
    );
    let cmd = some_cmd();
    assert_eq!(
        cr.load(CommandRef::from(&cmd)).await.unwrap(),
        CommandState::Redundant
    );
    assert!(repo.loaded().is_empty());
}

#[tokio::test]
async fn must_use_its_own_snapshot_if_not_empty() {
    let repo = Arc::new(Repo::new(persisted_state()));
    let cr = Cached::new(
        repo.clone(),
        Arc::new(BlackHoleCommandStore),
        Arc::new(ConstantSnapshotStore {
            state: 10,
            version: 3,
        }),
    );
    let cmd = some_cmd();
    assert_eq!(
        cr.load(CommandRef::from(&cmd)).await.unwrap(),
        in_mem_state()
    );
    assert!(repo.loaded().is_empty());
}

#[tokio::test]
async fn must_not_use_cold_snapshot() {
    let repo = Arc::new(Repo::new(persisted_state()));
    let cr = Cached::new(
        repo.clone(),
        Arc::new(BlackHoleCommandStore),
        Arc::new(LaggedSnapshotStore {
            state: 10,
            version: 3,
            lagged: 1,
        }),
    );
    let cmd = some_cmd();
    assert_eq!(
        cr.load(CommandRef::from(&cmd)).await.unwrap(),
        in_mem_state()
    );
    assert!(repo.loaded().is_empty());
}

#[tokio::test]
async fn must_update_its_commands_and_states_on_successful_append() {
    let events: NonEmpty<i32> = nonempty![1, 2, 3];
    let notifs = vec![4, 5, 6];
    let new_state = 11;
    let version: SeqNr = 1;
    let s = Arc::new(FakeSnapshotStore::<i32>::new());
    let c = Arc::new(FakeCommandStore::new());
    let repo = Arc::new(Repo::new(persisted_state()));
    let cr = Cached::new(repo.clone(), c.clone(), s.clone());
    let cmd = some_cmd();
    cr.append(
        CommandRef::from(&cmd),
        version,
        new_state,
        events.clone(),
        notifs.clone(),
    )
    .await
    .unwrap();
    assert_eq!(
        s.all(),
        HashMap::from([(
            cmd.address.clone(),
            ValidState::new(new_state, version + events.len() as SeqNr)
        )])
    );
    assert_eq!(c.all(), [cmd.id.clone()].into_iter().collect());
    assert_eq!(
        repo.actions(),
        vec![Action::Appended {
            cmd: erased(&cmd),
            version,
            new_state,
            events,
            notifications: notifs,
        }]
    );
}

#[tokio::test]
async fn must_not_update_its_commands_and_states_on_failed_append() {
    let s = Arc::new(FakeSnapshotStore::<i32>::new());
    let c = Arc::new(FakeCommandStore::new());
    let cr = Cached::new(Arc::new(FailingRepository), c.clone(), s.clone());
    let cmd = some_cmd();
    let result = cr
        .append(
            CommandRef::from(&cmd),
            1,
            11,
            nonempty![1, 2, 3],
            vec![4, 5, 6],
        )
        .await;
    assert_eq!(result, Err(planned_failure()));
    assert!(s.all().is_empty());
    assert!(c.all().is_empty());
}

#[tokio::test]
async fn must_notify_using_underlying() {
    let notifs = nonempty![4, 5, 6];
    let s = Arc::new(FakeSnapshotStore::<i32>::new());
    let c = Arc::new(FakeCommandStore::new());
    let repo = Arc::new(Repo::new(persisted_state()));
    let cr = Cached::new(repo.clone(), c.clone(), s.clone());
    let cmd = some_cmd();
    cr.notify(CommandRef::from(&cmd), notifs.clone())
        .await
        .unwrap();
    assert!(s.all().is_empty());
    assert!(c.all().is_empty());
    assert_eq!(
        repo.actions(),
        vec![Action::Notified {
            cmd: erased(&cmd),
            notifications: notifs,
        }]
    );
}
