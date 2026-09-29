//! Tenant-scoped reads.

use std::sync::Arc;

use async_trait::async_trait;
use edomata_core::BoxFuture;

use crate::{AuthPolicy, TenantId};

/// Reads a single entity on behalf of a caller; implementations filter by
/// the caller's tenant, so an entity of another tenant reads as missing.
///
/// ```
/// use std::collections::HashMap;
/// use edomata_saas::{AuthPolicy, CallerIdentity, CrudState, PermissivePolicy, TenantAwareReader};
///
/// struct Todos(HashMap<String, CrudState<String>>);
///
/// #[async_trait::async_trait]
/// impl TenantAwareReader<CallerIdentity, String> for Todos {
///     async fn get(&self, auth: &CallerIdentity, id: &str) -> Option<String> {
///         let tenant = PermissivePolicy.tenant_id(auth);
///         match self.0.get(id)? {
///             CrudState::Active { tenant_id, data, .. } if *tenant_id == tenant => Some(data.clone()),
///             _ => None,
///         }
///     }
/// }
///
/// let todos = Todos(HashMap::from([("t1".to_string(), CrudState::active("acme", "alice", "milk".to_string()))]));
/// let alice = CallerIdentity::new("acme", "alice", Vec::<String>::new());
/// let mallory = CallerIdentity::new("evil", "mallory", Vec::<String>::new());
/// futures::executor::block_on(async {
///     assert_eq!(todos.get(&alice, "t1").await.as_deref(), Some("milk"));
///     assert_eq!(todos.get(&mallory, "t1").await, None);
/// });
/// ```
#[async_trait]
pub trait TenantAwareReader<Auth, A>: Send + Sync {
    /// The entity, if it exists and belongs to the caller's tenant.
    async fn get(&self, auth: &Auth, entity_id: &str) -> Option<A>;
}

/// A query whose results are scoped to the caller's tenant. The `Auth`
/// parameter is structurally required, so tenant filtering cannot be
/// forgotten.
#[async_trait]
pub trait TenantScopedQuery<Auth, A, Q>: Send + Sync {
    /// Runs the query for the caller's tenant.
    async fn query(&self, auth: &Auth, q: Q) -> Vec<A>;
}

/// A query across all tenants. The `unsafe` prefix flags usage in code
/// reviews; use it only for administrative endpoints.
#[async_trait]
pub trait UnsafeCrossTenantQuery<A, Q>: Send + Sync {
    /// Runs the query.
    async fn query(&self, q: Q) -> Vec<A>;
}

type ScopedRun<A, Q> = dyn Fn(TenantId, Q) -> BoxFuture<'static, Vec<A>> + Send + Sync;
type CrossRun<A, Q> = dyn Fn(Q) -> BoxFuture<'static, Vec<A>> + Send + Sync;

/// A [`TenantScopedQuery`] built from a policy and a function of the
/// tenant (Scala's `TenantScopedQuery.apply`): the function receives the
/// tenant extracted by the policy, never the raw auth context.
///
/// ```
/// use edomata_saas::{CallerIdentity, PermissivePolicy, ScopedQueryFn, TenantScopedQuery};
///
/// let query = ScopedQueryFn::new(PermissivePolicy, |tenant, prefix: String| async move {
///     vec![format!("{prefix} of {tenant}")]
/// });
/// let caller = CallerIdentity::new("acme", "alice", Vec::<String>::new());
/// let rows = futures::executor::block_on(query.query(&caller, "todos".to_string()));
/// assert_eq!(rows, ["todos of acme"]);
/// ```
pub struct ScopedQueryFn<Auth, A, Q> {
    policy: Arc<dyn AuthPolicy<Auth>>,
    run: Arc<ScopedRun<A, Q>>,
}

impl<Auth, A, Q> ScopedQueryFn<Auth, A, Q> {
    /// Builds the query.
    pub fn new<P, F, Fut>(policy: P, run: F) -> Self
    where
        P: AuthPolicy<Auth> + 'static,
        F: Fn(TenantId, Q) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Vec<A>> + Send + 'static,
    {
        Self {
            policy: Arc::new(policy),
            run: Arc::new(move |tid, q| Box::pin(run(tid, q))),
        }
    }
}

#[async_trait]
impl<Auth, A, Q> TenantScopedQuery<Auth, A, Q> for ScopedQueryFn<Auth, A, Q>
where
    Auth: Sync,
    A: Send,
    Q: Send + 'static,
{
    async fn query(&self, auth: &Auth, q: Q) -> Vec<A> {
        (self.run)(self.policy.tenant_id(auth), q).await
    }
}

/// An [`UnsafeCrossTenantQuery`] built from a function (Scala's
/// `UnsafeCrossTenantQuery.apply`).
pub struct CrossTenantQueryFn<A, Q> {
    run: Arc<CrossRun<A, Q>>,
}

impl<A, Q> CrossTenantQueryFn<A, Q> {
    /// Builds the query.
    pub fn new<F, Fut>(run: F) -> Self
    where
        F: Fn(Q) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Vec<A>> + Send + 'static,
    {
        Self {
            run: Arc::new(move |q| Box::pin(run(q))),
        }
    }
}

#[async_trait]
impl<A, Q> UnsafeCrossTenantQuery<A, Q> for CrossTenantQueryFn<A, Q>
where
    A: Send,
    Q: Send + 'static,
{
    async fn query(&self, q: Q) -> Vec<A> {
        (self.run)(q).await
    }
}
