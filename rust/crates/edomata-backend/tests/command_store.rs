//! Port of `InMemoryCommandStoreSuite.scala`.

use edomata_backend::{CommandStore, InMemoryCommandStore};

#[tokio::test]
async fn must_add() {
    let ics = InMemoryCommandStore::new(1);
    ics.append("a").await.unwrap();
    assert!(ics.contains("a").await.unwrap());
}

#[tokio::test]
async fn must_remove_when_cache_size_is_reached() {
    let ics = InMemoryCommandStore::new(1);
    ics.append("a").await.unwrap();
    ics.append("b").await.unwrap();
    assert!(!ics.contains("a").await.unwrap());
    assert!(ics.contains("b").await.unwrap());
}

#[tokio::test]
async fn appending_twice_keeps_the_command() {
    let ics = InMemoryCommandStore::new(2);
    ics.append("a").await.unwrap();
    ics.append("b").await.unwrap();
    ics.append("a").await.unwrap();
    ics.append("c").await.unwrap();
    assert!(ics.contains("a").await.unwrap());
    assert!(!ics.contains("b").await.unwrap());
    assert!(ics.contains("c").await.unwrap());
}
