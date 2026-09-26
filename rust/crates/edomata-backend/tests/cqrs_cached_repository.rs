//! Port of `cqrs/CachedRepositorySuite.scala`.

mod common;

use std::sync::Arc;

use common::*;
use edomata_backend::cqrs::{
    AggregateState, CachedRepository, CommandState, Repository, RepositoryReader,
};
use edomata_backend::{Cache, CommandRef, LruCache};
use edomata_core::{CommandMessage, nonempty};

type Und = CqrsFakeRepository<String, i32>;
type Cached = CachedRepository<String, i32>;

fn some_cmd() -> CommandMessage<i32> {
    CommandMessage::new("cmdId", max_time(), "sut", 1)
}

fn und(state: AggregateState<String>) -> Arc<Und> {
    Arc::new(Und::new(CommandState::Aggregate(state)))
}

fn agg(s: &str, v: i64) -> AggregateState<String> {
    AggregateState::new(s.to_string(), v)
}

#[tokio::test]
async fn save_must_use_underlying_and_also_update_caches() {
    let und = und(agg("", 1));
    let cache = Arc::new(LruCache::<String, AggregateState<String>>::new(1));
    let cmds = Arc::new(FakeCommandStore::new());
    let repo = Cached::new(und.clone(), cmds.clone(), cache.clone());
    let cmd = some_cmd();
    repo.save(CommandRef::from(&cmd), 2, "state".into(), vec![4, 5, 6])
        .await
        .unwrap();
    assert_eq!(
        und.saved(),
        vec![Interaction::Saved {
            cmd: erased(&cmd),
            version: 2,
            new_state: "state".into(),
            events: vec![4, 5, 6],
        }]
    );
    assert_eq!(
        Cache::get(&*cache, &"sut".to_string()).await,
        Some(agg("state", 3))
    );
    assert_eq!(cmds.all(), ["cmdId".to_string()].into_iter().collect());
}

#[tokio::test]
async fn save_must_not_update_when_underlying_fails() {
    let cache = Arc::new(LruCache::<String, AggregateState<String>>::new(1));
    let cmds = Arc::new(FakeCommandStore::new());
    let repo = Cached::new(Arc::new(CqrsFailingRepository), cmds.clone(), cache.clone());
    let cmd = some_cmd();
    let result = repo
        .save(CommandRef::from(&cmd), 2, "state".into(), vec![4, 5, 6])
        .await;
    assert_eq!(result, Err(planned_failure()));
    assert_eq!(Cache::get(&*cache, &"sut".to_string()).await, None);
    assert!(cmds.all().is_empty());
}

#[tokio::test]
async fn cache_updating_must_be_convergent() {
    let und = und(agg("", 1));
    let cache = Arc::new(LruCache::<String, AggregateState<String>>::new(1));
    let cmds = Arc::new(FakeCommandStore::new());
    let repo = Cached::new(und.clone(), cmds.clone(), cache.clone());
    let mut new_cmd = some_cmd();
    new_cmd.id = "new".into();
    let mut old_cmd = some_cmd();
    old_cmd.id = "old".into();
    repo.save(CommandRef::from(&new_cmd), 3, "state new".into(), vec![])
        .await
        .unwrap();
    repo.save(
        CommandRef::from(&old_cmd),
        2,
        "state old".into(),
        vec![4, 5, 6],
    )
    .await
    .unwrap();
    assert_eq!(
        Cache::get(&*cache, &"sut".to_string()).await,
        Some(agg("state new", 4))
    );
    assert_eq!(
        cmds.all(),
        ["new".to_string(), "old".to_string()].into_iter().collect()
    );
}

#[tokio::test]
async fn get_must_use_underlying() {
    let und = und(agg("", 1));
    let repo = Cached::build(und.clone(), 1000, 1000);
    let cmd = some_cmd();
    repo.save(CommandRef::from(&cmd), 2, "new state".into(), vec![])
        .await
        .unwrap();
    assert_eq!(repo.get("sut").await.unwrap(), agg("", 1));
}

#[tokio::test]
async fn notify_must_use_underlying() {
    let und = und(agg("", 1));
    let repo = Cached::build(und.clone(), 1000, 1000);
    let cmd = some_cmd();
    repo.notify(CommandRef::from(&cmd), nonempty![1, 2, 3])
        .await
        .unwrap();
    assert_eq!(
        und.saved(),
        vec![Interaction::Notified {
            cmd: erased(&cmd),
            events: nonempty![1, 2, 3],
        }]
    );
}

#[tokio::test]
async fn load_must_ignore_underlying_if_cache_has_the_required_data() {
    let und = und(agg("", 1));
    let repo = Cached::build(und.clone(), 1000, 1000);
    let cmd = some_cmd();
    repo.save(CommandRef::from(&cmd), 2, "new state".into(), vec![])
        .await
        .unwrap();
    assert_eq!(
        repo.load(CommandRef::from(&cmd)).await.unwrap(),
        CommandState::Redundant
    );
    let mut other = some_cmd();
    other.id = "new cmdId".into();
    assert_eq!(
        repo.load(CommandRef::from(&other)).await.unwrap(),
        CommandState::Aggregate(agg("new state", 3))
    );
}

#[tokio::test]
async fn load_must_use_underlying_if_cache_does_not_have_the_required_data() {
    let und = und(agg("underlying", 4));
    let repo = Cached::build(und.clone(), 1000, 1000);
    let cmd = some_cmd();
    assert_eq!(
        repo.load(CommandRef::from(&cmd)).await.unwrap(),
        CommandState::Aggregate(agg("underlying", 4))
    );
}
