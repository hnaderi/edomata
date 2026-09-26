//! Port of `TenantAwareReaderSuite.scala`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use edomata_saas::{
    CallerIdentity, CrossTenantQueryFn, RoleBasedPolicy, ScopedQueryFn, TenantAwareReader,
    TenantId, TenantScopedQuery, UnsafeCrossTenantQuery,
};
use futures::executor::block_on;

fn caller_a() -> CallerIdentity {
    CallerIdentity::new("tenant-a", "user-a", ["read"])
}
fn caller_b() -> CallerIdentity {
    CallerIdentity::new("tenant-b", "user-a", ["read"])
}

#[test]
fn scoped_query_passes_callers_tenant_to_the_function() {
    let captured = Arc::new(Mutex::new(TenantId::new("")));
    let c = Arc::clone(&captured);
    let query: ScopedQueryFn<CallerIdentity, String, ()> =
        ScopedQueryFn::new(RoleBasedPolicy::none(), move |tid, ()| {
            let c = Arc::clone(&c);
            async move {
                *c.lock().unwrap() = tid;
                vec!["item1".to_string(), "item2".to_string()]
            }
        });
    let result = block_on(query.query(&caller_a(), ()));
    assert_eq!(*captured.lock().unwrap(), TenantId::new("tenant-a"));
    assert_eq!(result, vec!["item1".to_string(), "item2".to_string()]);
}

#[test]
fn scoped_query_different_callers_get_different_filters() {
    let query: ScopedQueryFn<CallerIdentity, String, ()> =
        ScopedQueryFn::new(RoleBasedPolicy::none(), |tid, ()| async move {
            vec![format!("from-{}", tid.value())]
        });
    assert_eq!(
        block_on(query.query(&caller_a(), ())),
        vec!["from-tenant-a".to_string()]
    );
    assert_eq!(
        block_on(query.query(&caller_b(), ())),
        vec!["from-tenant-b".to_string()]
    );
}

#[test]
fn scoped_query_passes_the_query_parameter_through() {
    let captured = Arc::new(Mutex::new(String::new()));
    let c = Arc::clone(&captured);
    let query: ScopedQueryFn<CallerIdentity, String, String> =
        ScopedQueryFn::new(RoleBasedPolicy::none(), move |_, q| {
            let c = Arc::clone(&c);
            async move {
                *c.lock().unwrap() = q;
                Vec::new()
            }
        });
    block_on(query.query(&caller_a(), "search-term".to_string()));
    assert_eq!(*captured.lock().unwrap(), "search-term");
}

#[test]
fn scoped_query_returns_empty_when_no_results() {
    let query: ScopedQueryFn<CallerIdentity, String, ()> =
        ScopedQueryFn::new(RoleBasedPolicy::none(), |_, ()| async { Vec::new() });
    assert_eq!(block_on(query.query(&caller_a(), ())), Vec::<String>::new());
}

#[test]
fn cross_tenant_query_does_not_require_a_caller() {
    let query: CrossTenantQueryFn<String, ()> = CrossTenantQueryFn::new(|()| async {
        vec![
            "all-tenants-item1".to_string(),
            "all-tenants-item2".to_string(),
        ]
    });
    assert_eq!(
        block_on(query.query(())),
        vec![
            "all-tenants-item1".to_string(),
            "all-tenants-item2".to_string()
        ]
    );
}

#[test]
fn cross_tenant_query_passes_the_query_parameter_through() {
    let captured = Arc::new(Mutex::new(0));
    let c = Arc::clone(&captured);
    let query: CrossTenantQueryFn<String, i32> = CrossTenantQueryFn::new(move |q| {
        let c = Arc::clone(&c);
        async move {
            *c.lock().unwrap() = q;
            Vec::new()
        }
    });
    block_on(query.query(42));
    assert_eq!(*captured.lock().unwrap(), 42);
}

#[test]
fn cross_tenant_query_returns_empty_when_no_results() {
    let query: CrossTenantQueryFn<String, ()> = CrossTenantQueryFn::new(|()| async { Vec::new() });
    assert_eq!(block_on(query.query(())), Vec::<String>::new());
}

struct MapReader {
    store: HashMap<(String, String), String>,
}

#[async_trait]
impl TenantAwareReader<CallerIdentity, String> for MapReader {
    async fn get(&self, auth: &CallerIdentity, entity_id: &str) -> Option<String> {
        self.store
            .get(&(auth.tenant_id.value().to_string(), entity_id.to_string()))
            .cloned()
    }
}

#[test]
fn tenant_aware_reader_can_be_implemented_for_single_entity_reads() {
    let reader = MapReader {
        store: HashMap::from([
            (
                ("tenant-a".to_string(), "e1".to_string()),
                "data-1".to_string(),
            ),
            (
                ("tenant-b".to_string(), "e2".to_string()),
                "data-2".to_string(),
            ),
        ]),
    };
    assert_eq!(
        block_on(reader.get(&caller_a(), "e1")),
        Some("data-1".to_string())
    );
    assert_eq!(block_on(reader.get(&caller_a(), "e2")), None);
    assert_eq!(
        block_on(reader.get(&caller_b(), "e2")),
        Some("data-2".to_string())
    );
    assert_eq!(block_on(reader.get(&caller_b(), "e1")), None);
}
