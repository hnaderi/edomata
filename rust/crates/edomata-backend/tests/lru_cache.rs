//! Port of `LRUCacheSuite.scala`.

use edomata_backend::{Cache, LruCache};

fn empty_cache() -> LruCache<i32, i32> {
    LruCache::new(3)
}

fn new_cache() -> LruCache<i32, i32> {
    let c = empty_cache();
    for i in 1..=3 {
        c.insert(i, i * 2);
    }
    c
}

#[test]
fn must_store_whats_added() {
    let c = empty_cache();
    assert_eq!(c.insert(1, 2), None);
    assert_eq!(c.lookup(&1), Some(2));
    assert_eq!(c.entries(), vec![(1, 2)]);
    assert_eq!(c.len(), 1);
    assert_eq!(c.max_size(), 3);
}

#[test]
fn must_evict_least_recently_used_item_when_reaches_its_max_size() {
    let c = new_cache();
    assert_eq!(c.insert(4, 8), Some((1, 2)));
    let i1 = c.by_usage();
    assert_eq!(c.lookup(&1), None);
    let i2 = c.by_usage();
    assert_eq!(i1, i2);
    assert_eq!(i1, vec![(4, 8), (3, 6), (2, 4)]);
}

#[test]
fn must_keep_recently_used_items_fresh() {
    let c = new_cache();
    assert_eq!(c.values_by_usage(), vec![6, 4, 2]);
    assert_eq!(c.lookup(&2), Some(4));
    assert_eq!(c.values_by_usage(), vec![4, 6, 2]);
    assert_eq!(c.lookup(&3), Some(6));
    assert_eq!(c.values_by_usage(), vec![6, 4, 2]);
    assert_eq!(c.lookup(&1), Some(2));
    assert_eq!(c.values_by_usage(), vec![2, 6, 4]);
    assert_eq!(c.first_value(), Some(2));
    assert_eq!(c.last_value(), Some(4));
}

#[test]
fn must_replace_if_predicate_matches() {
    let c = new_cache();
    c.insert_if(1, 3, |v| *v == 2);
    assert_eq!(c.lookup(&1), Some(3));
}

#[test]
fn must_not_replace_if_predicate_does_not_match() {
    let c = new_cache();
    c.insert_if(1, 3, |v| *v == 4);
    assert_eq!(c.lookup(&1), Some(2));
}

#[test]
fn must_add_when_replacing_a_non_existing_key() {
    let c = new_cache();
    c.insert_if(4, 5, |v| *v == 4);
    assert_eq!(c.lookup(&4), Some(5));
}

#[test]
fn peek_does_not_touch_usage() {
    let c = new_cache();
    assert_eq!(c.peek(&1), Some(2));
    assert_eq!(c.values_by_usage(), vec![6, 4, 2]);
}

#[tokio::test]
async fn cache_trait_delegates() {
    let c = new_cache();
    assert_eq!(Cache::add(&c, 4, 8).await, Some((1, 2)));
    assert_eq!(Cache::get(&c, &4).await, Some(8));
    assert_eq!(Cache::replace(&c, 4, 9, &|v| *v == 8).await, None);
    assert_eq!(Cache::get(&c, &4).await, Some(9));
    assert_eq!(Cache::get(&c, &1).await, None);
}
