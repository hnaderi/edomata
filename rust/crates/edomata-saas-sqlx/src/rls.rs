//! The single entry point of the Row-Level Security contract.

use std::future::Future;
use std::sync::Arc;

use async_trait::async_trait;
use edomata_backend::BackendError;
use edomata_postgres::PGNaming;
use edomata_saas::{Access, RlsAccessPolicy, TenantId};
use edomata_sqlx::shared::map_sqlx;
use edomata_sqlx::{SqlxDriver, TxScope};
use futures::future::BoxFuture;
use sqlx::{PgConnection, PgPool, Postgres, Transaction};

tokio::task_local! {
    static CURRENT: Access;
}

/// The single entry point to the event-sourced tables of a SaaS service
/// under Row-Level Security.
///
/// Every transaction that touches them starts in [`begin`](Self::begin),
/// which, inside the transaction:
///
/// 1. switches to the role of the access (`SET LOCAL ROLE`; PostgreSQL
///    refuses it unless the connection's login is a **member** of that
///    role, which is what keeps the system and inspection modes out of
///    reach of a login that was not granted them), then
/// 2. sets the tenant with `set_config(setting, value, true)`:
///    **transaction-local**, so it ends with the transaction and a pooled
///    connection never carries a tenant into its next use. A missing or
///    empty tenant matches no row.
///
/// Nothing outside this type sets the role or the tenant, and nothing in
/// the policies trusts a setting the application can change freely: the
/// tenant setting only selects rows *within* the tenant role.
///
/// ```no_run
/// # use edomata_saas::{PGNaming, RlsAccessPolicy, TenantId};
/// # use edomata_saas_sqlx::RlsGateway;
/// # async fn example(pool: sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
/// let policy: RlsAccessPolicy<TenantId> = RlsAccessPolicy::with_prefix("shop")?;
/// let gateway = RlsGateway::new(pool, policy.clone());
///
/// // Raw SQL, as tenant "acme": the transaction is committed on `Ok`.
/// let access = policy.tenant_access(&TenantId::new("acme"))?;
/// let visible: i64 = gateway
///     .run(&access, |conn| {
///         Box::pin(async move {
///             sqlx::query_scalar("select count(*) from shop_journal")
///                 .fetch_one(conn)
///                 .await
///                 .map_err(edomata_backend::BackendError::unknown)
///         })
///     })
///     .await?;
///
/// // A backend: every transaction of its driver runs as the tenant role,
/// // for the access of the surrounding `with_access`.
/// let driver = gateway.driver(PGNaming::prefixed_str("shop")?).await?;
/// # let _ = (visible, driver); Ok(()) }
/// ```
pub struct RlsGateway<T = TenantId> {
    pool: PgPool,
    policy: RlsAccessPolicy<T>,
}

impl<T> Clone for RlsGateway<T> {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            policy: self.policy.clone(),
        }
    }
}

impl<T> std::fmt::Debug for RlsGateway<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RlsGateway")
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

impl<T> RlsGateway<T> {
    /// A gateway over `pool`. The pool's login must be a member of the
    /// roles of the modes it will use
    /// ([`SaaSPGSchema::rls_memberships`](edomata_saas::SaaSPGSchema::rls_memberships)).
    pub fn new(pool: PgPool, policy: RlsAccessPolicy<T>) -> Self {
        Self { pool, policy }
    }

    /// The access policy.
    pub fn policy(&self) -> &RlsAccessPolicy<T> {
        &self.policy
    }

    /// The pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Opens a transaction, switches to the role of `access` and injects
    /// the tenant (see the [type](Self) documentation). The transaction
    /// rolls back when dropped; commit it to keep its writes.
    ///
    /// # Errors
    ///
    /// [`BackendError::PersistenceError`] when `access` does not come from
    /// this gateway's policy; [`BackendError::UnknownError`] when PostgreSQL
    /// refuses (for example, the login is not a member of the role).
    pub async fn begin(
        &self,
        access: &Access,
    ) -> Result<Transaction<'static, Postgres>, BackendError> {
        begin_access(&self.pool, &self.policy, access).await
    }

    /// Runs `f` in a transaction opened by [`begin`](Self::begin) and
    /// closes it: committed when `f` returns `Ok`, rolled back otherwise.
    ///
    /// # Errors
    ///
    /// The errors of [`begin`](Self::begin), of `f`, and of the commit.
    pub async fn run<F, V>(&self, access: &Access, f: F) -> Result<V, BackendError>
    where
        F: for<'c> FnOnce(&'c mut PgConnection) -> BoxFuture<'c, Result<V, BackendError>>,
    {
        let mut tx = self.begin(access).await?;
        match f(&mut tx).await {
            Ok(value) => {
                tx.commit().await.map_err(map_sqlx)?;
                Ok(value)
            }
            Err(e) => {
                // Dropping would roll back too; be explicit.
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }

    /// Runs `future` with `access` as the ambient access of every driver
    /// built by [`driver`](Self::driver) (and of
    /// [`tx_scope`](Self::tx_scope)): the web tier calls it once per
    /// request, with the authenticated tenant, around `service.handle(..)`.
    /// The access is a task-local value: it does not follow
    /// `tokio::spawn`, and a driver used outside any `with_access` fails
    /// instead of running unscoped.
    pub async fn with_access<Fut: Future>(&self, access: &Access, future: Fut) -> Fut::Output {
        CURRENT.scope(access.clone(), future).await
    }

    /// The [`TxScope`] reading the ambient access set by
    /// [`with_access`](Self::with_access).
    pub fn tx_scope(&self) -> Arc<dyn TxScope> {
        Arc::new(GatewayScope {
            policy: self.policy.retyped(),
            fixed: None,
        })
    }

    /// A [`TxScope`] that always uses `access`, whatever the ambient
    /// access: for the background writes of a persisted snapshot store
    /// ([`SqlxDriver::with_snapshot_write_scope`]), which have no caller.
    /// Using the system mode there needs the login to be a member of the
    /// system role.
    pub fn fixed_scope(&self, access: Access) -> Arc<dyn TxScope> {
        Arc::new(GatewayScope {
            policy: self.policy.retyped(),
            fixed: Some(access),
        })
    }

    /// An event-sourcing driver whose every transaction (journal, outbox,
    /// commands, snapshots; reads included) goes through
    /// [`begin`](Self::begin) for the ambient access of
    /// [`with_access`](Self::with_access). No DDL is run: the tables
    /// belong to the migration role ([`SaaSPGSchema::eventsourcing_rls`](edomata_saas::SaaSPGSchema::eventsourcing_rls)).
    ///
    /// # Errors
    ///
    /// Never in practice (no DDL is executed); the signature follows
    /// [`SqlxDriver::new_with`].
    pub async fn driver(&self, naming: PGNaming) -> Result<SqlxDriver, BackendError> {
        Ok(SqlxDriver::new_with(naming, self.pool.clone(), true)
            .await?
            .with_tx_scope(self.tx_scope()))
    }
}

async fn begin_access<T>(
    pool: &PgPool,
    policy: &RlsAccessPolicy<T>,
    access: &Access,
) -> Result<Transaction<'static, Postgres>, BackendError> {
    if access.tenant_setting() != policy.tenant_setting()
        || [
            policy.tenant_role(),
            policy.system_role(),
            policy.inspection_role(),
        ]
        .iter()
        .all(|role| *role != access.role())
    {
        return Err(BackendError::persistence(
            "the access does not come from this gateway's access policy",
        ));
    }
    let mut tx = pool.begin().await.map_err(map_sqlx)?;
    sqlx::query(&access.set_role_statement())
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    // `true`: local to this transaction.
    sqlx::query("select set_config($1, $2, true)")
        .bind(access.tenant_setting())
        .bind(access.tenant_setting_value())
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx)?;
    Ok(tx)
}

#[derive(Debug)]
struct GatewayScope {
    policy: RlsAccessPolicy<()>,
    fixed: Option<Access>,
}

#[async_trait]
impl TxScope for GatewayScope {
    async fn begin(&self, pool: &PgPool) -> Result<Transaction<'static, Postgres>, BackendError> {
        let access = match &self.fixed {
            Some(access) => access.clone(),
            None => CURRENT.try_with(Access::clone).map_err(|_| {
                BackendError::persistence(
                    "no access context: run the call inside `RlsGateway::with_access`",
                )
            })?,
        };
        begin_access(pool, &self.policy, &access).await
    }
}
