//! Row-Level Security of the event-sourced SaaS tables against a real
//! PostgreSQL: the generated DDL, the three access modes, the single entry
//! point (`RlsGateway`) and the guarded service running as the tenant role.
//!
//! The database comes from `EDOMATA_TEST_DATABASE_URL`, else `DATABASE_URL`
//! (the PostgreSQL service of the CI), else the docker-compose default of
//! `rust/README.md`. The connecting user must be a superuser: the tests
//! create roles (`edomata_rlstest_*`) and one schema per test
//! (`edomata_rlstest_<test>`), and connect as *non-superuser logins* by
//! `SET SESSION AUTHORIZATION`, so that role membership is really checked.

use std::sync::Arc;

use async_trait::async_trait;
use edomata_backend::BackendError;
use edomata_backend::eventsourcing::{
    AggregateState, Backend, SnapshotReader, SnapshotStore, ValidState,
};
use edomata_core::{DomainModel, NonEmpty};
use edomata_saas::{
    Access, AccessMode, CallerIdentity, CrudAction, CrudState, Decision, PGNaming,
    PermissivePolicy, RlsAccessPolicy, SaaSCommand, SaaSEsApp, SaaSEventSourcedService,
    SaaSPGSchema, TenantId,
};
use edomata_saas_sqlx::RlsGateway;
use edomata_sqlx::SqlxCodec;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

const PREFIX: &str = "edomata_rlstest";
const OWNER: &str = "edomata_rlstest_owner";
/// Logins: the web tier (tenant only), a relay (system only), an inspection
/// agent (inspection only) and a process that may be tenant and system.
const WEB: &str = "edomata_rlstest_web";
const JOB: &str = "edomata_rlstest_job";
const AGENT: &str = "edomata_rlstest_agent";
const APP: &str = "edomata_rlstest_app";

fn policy() -> RlsAccessPolicy<TenantId> {
    RlsAccessPolicy::with_prefix(PREFIX).unwrap()
}

fn database_url() -> String {
    std::env::var("EDOMATA_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string())
}

async fn admin() -> PgPool {
    PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url())
        .await
        .expect("a superuser PostgreSQL connection is required (see rust/README.md)")
}

/// A pool whose connections are authenticated as `login`, a plain
/// non-superuser role (`SET SESSION AUTHORIZATION` from the superuser).
async fn login_pool(login: &'static str, max: u32) -> PgPool {
    PgPoolOptions::new()
        .max_connections(max)
        .after_connect(move |conn, _| {
            Box::pin(async move {
                sqlx::query(&format!("SET SESSION AUTHORIZATION {login}"))
                    .execute(conn)
                    .await?;
                Ok(())
            })
        })
        .connect(&database_url())
        .await
        .unwrap()
}

/// Creates the roles, the owner and the logins (idempotent, serialised by an
/// advisory lock: the tests of this file run in parallel).
async fn ensure_roles(admin: &PgPool) {
    let mut conn = admin.acquire().await.unwrap();
    sqlx::query("select pg_advisory_lock(7201001)")
        .execute(&mut *conn)
        .await
        .unwrap();
    let policy = policy();
    let mut statements = SaaSPGSchema::rls_roles(&policy);
    for (role, attributes) in [
        (OWNER, "NOLOGIN"),
        (WEB, "LOGIN NOINHERIT"),
        (JOB, "LOGIN NOINHERIT"),
        (AGENT, "LOGIN NOINHERIT"),
        (APP, "LOGIN NOINHERIT"),
    ] {
        statements.push(format!(
            "DO $$ BEGIN IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = '{role}') THEN CREATE ROLE {role} {attributes}; END IF; END $$;"
        ));
    }
    for (login, modes) in [
        (WEB, vec![AccessMode::Tenant]),
        (JOB, vec![AccessMode::System]),
        (AGENT, vec![AccessMode::Inspection]),
        (APP, vec![AccessMode::Tenant, AccessMode::System]),
    ] {
        statements.extend(SaaSPGSchema::rls_memberships(&policy, login, &modes).unwrap());
    }
    for statement in statements {
        sqlx::query(&statement).execute(&mut *conn).await.unwrap();
    }
    sqlx::query("select pg_advisory_unlock(7201001)")
        .execute(&mut *conn)
        .await
        .unwrap();
}

/// One schema per test, owned by the owner role, with the tables, the RLS
/// statements and the grants of `SaaSPGSchema::eventsourcing_rls`, applied
/// as the owner (the application roles own nothing).
struct Fixture {
    admin: PgPool,
    schema: String,
}

impl Fixture {
    async fn new(test: &str) -> Self {
        let admin = admin().await;
        ensure_roles(&admin).await;
        let schema = format!("{PREFIX}_{test}");
        sqlx::query(&format!("DROP SCHEMA IF EXISTS \"{schema}\" CASCADE"))
            .execute(&admin)
            .await
            .unwrap();
        sqlx::query(&format!("CREATE SCHEMA \"{schema}\" AUTHORIZATION {OWNER}"))
            .execute(&admin)
            .await
            .unwrap();
        let ddl = SaaSPGSchema::eventsourcing_rls(
            &PGNaming::schema_str(&schema).unwrap(),
            "jsonb",
            "jsonb",
            "jsonb",
            &policy(),
        );
        let mut conn = admin.acquire().await.unwrap();
        sqlx::query(&format!("SET ROLE {OWNER}"))
            .execute(&mut *conn)
            .await
            .unwrap();
        // The first statement is `CREATE SCHEMA IF NOT EXISTS`, which the
        // owner may not run (no CREATE on the database); the schema was
        // created above, for the owner.
        for statement in ddl.iter().skip(1) {
            sqlx::query(statement)
                .execute(&mut *conn)
                .await
                .unwrap_or_else(|e| panic!("{e}: {statement}"));
        }
        sqlx::query("RESET ROLE").execute(&mut *conn).await.unwrap();
        drop(conn);
        Self { admin, schema }
    }

    fn naming(&self) -> PGNaming {
        PGNaming::schema_str(&self.schema).unwrap()
    }

    fn table(&self, name: &str) -> String {
        format!("\"{}\".{name}", self.schema)
    }

    async fn cleanup(self) {
        sqlx::query(&format!(
            "DROP SCHEMA IF EXISTS \"{}\" CASCADE",
            self.schema
        ))
        .execute(&self.admin)
        .await
        .unwrap();
    }
}

const TABLES: [&str; 4] = ["journal", "outbox", "commands", "snapshots"];

fn tenant(policy: &RlsAccessPolicy<TenantId>, name: &str) -> Access {
    policy.tenant_access(&TenantId::new(name)).unwrap()
}

/// Runs one statement through the entry point; the rows affected.
async fn exec(
    gateway: &RlsGateway,
    access: &Access,
    sql: String,
    bind: Vec<String>,
) -> Result<u64, BackendError> {
    gateway
        .run(access, |conn| {
            Box::pin(async move {
                let mut q = sqlx::query(&sql);
                for b in &bind {
                    q = q.bind(b);
                }
                q.execute(conn)
                    .await
                    .map(|r| r.rows_affected())
                    .map_err(BackendError::unknown)
            })
        })
        .await
}

async fn count(gateway: &RlsGateway, access: &Access, table: String) -> i64 {
    gateway
        .run(access, |conn| {
            Box::pin(async move {
                sqlx::query_scalar(&format!("select count(*) from {table}"))
                    .fetch_one(conn)
                    .await
                    .map_err(BackendError::unknown)
            })
        })
        .await
        .unwrap()
}

/// The `INSERT` of one row of `table` for the stream / command `stream`.
fn insert_sql(f: &Fixture, table: &str, stream: &str) -> (String, Vec<String>) {
    let t = f.table(table);
    let tenant = stream.split('/').next().unwrap_or_default();
    match table {
        "journal" => (
            format!(
                "insert into {t} (id, \"time\", version, stream, payload) values (gen_random_uuid(), now(), 1, $1, '{{}}'::jsonb)"
            ),
            vec![stream.into()],
        ),
        "outbox" => (
            format!("insert into {t} (stream, payload, created) values ($1, '{{}}'::jsonb, now())"),
            vec![stream.into()],
        ),
        "commands" => (
            format!("insert into {t} (id, \"time\", address) values ($1, now(), $2)"),
            vec![format!("{tenant}/c-{stream}"), stream.into()],
        ),
        "snapshots" => (
            format!("insert into {t} (id, version, state) values ($1, 1, '{{}}'::jsonb)"),
            vec![stream.into()],
        ),
        other => panic!("{other}"),
    }
}

/// One row per table for `tenant` (stream `{tenant}/s1`), as the tenant.
async fn seed(f: &Fixture, gateway: &RlsGateway, name: &str) {
    let access = tenant(gateway.policy(), name);
    for table in TABLES {
        let (sql, bind) = insert_sql(f, table, &format!("{name}/s1"));
        assert_eq!(exec(gateway, &access, sql, bind).await.unwrap(), 1);
    }
}

fn message(e: &BackendError) -> String {
    format!("{e:?} {e}")
}

// ---------------------------------------------------------------------------
// Tenant mode
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tenant_a_reads_and_writes_its_rows_and_not_tenant_bs() {
    let f = Fixture::new("tenant_isolation").await;
    let gateway = RlsGateway::new(login_pool(WEB, 4).await, policy());
    seed(&f, &gateway, "a").await;
    seed(&f, &gateway, "b").await;
    let a = tenant(gateway.policy(), "a");

    for table in TABLES {
        assert_eq!(count(&gateway, &a, f.table(table)).await, 1, "{table}");
    }
    let streams: Vec<String> = gateway
        .run(&a, |conn| {
            let sql = format!("select stream from {} order by 1", f.table("journal"));
            Box::pin(async move {
                sqlx::query_scalar(&sql)
                    .fetch_all(conn)
                    .await
                    .map_err(BackendError::unknown)
            })
        })
        .await
        .unwrap();
    assert_eq!(streams, vec!["a/s1".to_string()]);

    // Inserting for tenant B fails the WITH CHECK, on every table.
    for table in TABLES {
        let (sql, bind) = insert_sql(&f, table, "b/s2");
        let err = exec(&gateway, &a, sql, bind).await.unwrap_err();
        assert!(
            message(&err).contains("row-level security"),
            "{table}: {}",
            message(&err)
        );
    }
    // B's rows are not writable either: an update matches nothing (A's own
    // snapshot is updated).
    let update = |id: &'static str| {
        exec(
            &gateway,
            &a,
            format!(
                "update {} set version = 9 where id = $1",
                f.table("snapshots")
            ),
            vec![id.to_string()],
        )
    };
    assert_eq!(update("b/s1").await.unwrap(), 0);
    assert_eq!(update("a/s1").await.unwrap(), 1);
    // The tenant role has no DELETE (nor UPDATE on the journal).
    let err = exec(
        &gateway,
        &a,
        format!("delete from {} where stream = 'a/s1'", f.table("journal")),
        vec![],
    )
    .await
    .unwrap_err();
    assert!(
        message(&err).contains("permission denied"),
        "{}",
        message(&err)
    );
    // Tenant B still has its row, untouched.
    let b = tenant(gateway.policy(), "b");
    assert_eq!(count(&gateway, &b, f.table("snapshots")).await, 1);
    f.cleanup().await;
}

#[tokio::test]
async fn ids_without_a_tenant_prefix_or_with_a_foreign_address_are_refused() {
    let f = Fixture::new("malformed_ids").await;
    let gateway = RlsGateway::new(login_pool(APP, 4).await, policy());
    let a = tenant(gateway.policy(), "a");
    let system = gateway.policy().system_access();

    for table in ["journal", "outbox", "snapshots"] {
        // No `/`: no tenant can be derived, the NOT NULL column refuses it,
        // for the tenant and for the system role alike.
        for access in [&a, &system] {
            let (sql, bind) = insert_sql(&f, table, "plain");
            let err = exec(&gateway, access, sql, bind).await.unwrap_err();
            assert!(
                message(&err).contains("tenant_id") || message(&err).contains("row-level"),
                "{table}: {}",
                message(&err)
            );
        }
        // An empty tenant (`/x`) neither.
        let (sql, bind) = insert_sql(&f, table, "/x");
        assert!(exec(&gateway, &system, sql, bind).await.is_err(), "{table}");
    }
    // A command of tenant `a` that addresses a stream of tenant `b`: the
    // CHECK constraint refuses it for everyone, the system role included.
    let sql = format!(
        "insert into {} (id, \"time\", address) values ('a/c1', now(), 'b/s1')",
        f.table("commands")
    );
    let err = exec(&gateway, &system, sql, vec![]).await.unwrap_err();
    assert!(
        message(&err).contains("commands_address_tenant_ck"),
        "{}",
        message(&err)
    );
    f.cleanup().await;
}

#[tokio::test]
async fn without_a_tenant_or_with_an_empty_one_nothing_is_visible_or_writable() {
    let f = Fixture::new("no_tenant").await;
    let gateway = RlsGateway::new(login_pool(WEB, 4).await, policy());
    seed(&f, &gateway, "a").await;
    let setting = policy().tenant_setting().to_string();

    // Misuse, bypassing the entry point on purpose: the tenant role without
    // a tenant (never set on this connection) and with an empty one.
    for set_empty in [false, true] {
        // A fresh pool: on its connection the setting was never defined.
        let pool = login_pool(WEB, 1).await;
        for table in TABLES {
            let mut tx = pool.begin().await.unwrap();
            sqlx::query(&format!("SET LOCAL ROLE {PREFIX}_tenant"))
                .execute(&mut *tx)
                .await
                .unwrap();
            if set_empty {
                sqlx::query("select set_config($1, '', true)")
                    .bind(&setting)
                    .execute(&mut *tx)
                    .await
                    .unwrap();
            }
            let n: i64 = sqlx::query_scalar(&format!("select count(*) from {}", f.table(table)))
                .fetch_one(&mut *tx)
                .await
                .unwrap();
            assert_eq!(n, 0, "{table} (empty setting: {set_empty})");
            let (sql, bind) = insert_sql(&f, table, "a/s9");
            let mut q = sqlx::query(&sql);
            for b in &bind {
                q = q.bind(b);
            }
            let err = q.execute(&mut *tx).await.unwrap_err().to_string();
            assert!(err.contains("row-level security"), "{table}: {err}");
            tx.rollback().await.unwrap();
        }
    }
    f.cleanup().await;
}

#[tokio::test]
async fn the_tenant_of_one_transaction_is_gone_in_the_next_on_the_same_connection() {
    let f = Fixture::new("no_leak").await;
    // One connection only: the second transaction reuses the first's.
    let pool = login_pool(WEB, 1).await;
    let gateway = RlsGateway::new(pool.clone(), policy());
    seed(&f, &gateway, "a").await;
    let a = tenant(gateway.policy(), "a");

    let pid_a: i32 = gateway
        .run(&a, |conn| {
            Box::pin(async move {
                sqlx::query_scalar("select pg_backend_pid()")
                    .fetch_one(conn)
                    .await
                    .map_err(BackendError::unknown)
            })
        })
        .await
        .unwrap();
    assert_eq!(count(&gateway, &a, f.table("journal")).await, 1);

    // The next user of the connection, without the entry point.
    let mut tx = pool.begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(pid, pid_a, "the test needs the very same connection");
    sqlx::query(&format!("SET LOCAL ROLE {PREFIX}_tenant"))
        .execute(&mut *tx)
        .await
        .unwrap();
    let leaked: Option<String> = sqlx::query_scalar("select nullif(current_setting($1, true), '')")
        .bind(policy().tenant_setting())
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(leaked, None, "the tenant must not survive its transaction");
    let n: i64 = sqlx::query_scalar(&format!("select count(*) from {}", f.table("journal")))
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(n, 0);
    // Release the locks before the schema is dropped.
    tx.rollback().await.unwrap();
    f.cleanup().await;
}

#[tokio::test]
async fn a_tenant_cannot_switch_mode_with_a_session_variable_or_row_security() {
    let f = Fixture::new("no_mode_switch").await;
    let gateway = RlsGateway::new(login_pool(WEB, 4).await, policy());
    seed(&f, &gateway, "a").await;
    seed(&f, &gateway, "b").await;
    let a = tenant(gateway.policy(), "a");

    // Whatever flag a session sets, A sees A (counted after every attempt).
    let seen: Vec<i64> = gateway
        .run(&a, |conn| {
            let table = f.table("journal");
            Box::pin(async move {
                let mut seen = Vec::new();
                for name in [
                    "app.mode",
                    "app.role",
                    "app.system",
                    "edomata_rlstest.mode",
                    "edomata_rlstest.system",
                    "edomata_rlstest.bypass",
                ] {
                    for value in ["system", "bypass", "true", "on", "admin"] {
                        sqlx::query("select set_config($1, $2, true)")
                            .bind(name)
                            .bind(value)
                            .execute(&mut *conn)
                            .await
                            .map_err(BackendError::unknown)?;
                        seen.push(
                            sqlx::query_scalar(&format!("select count(*) from {table}"))
                                .fetch_one(&mut *conn)
                                .await
                                .map_err(BackendError::unknown)?,
                        );
                    }
                }
                Ok(seen)
            })
        })
        .await
        .unwrap();
    assert!(seen.iter().all(|n| *n == 1), "{seen:?}");

    // Turning `row_security` off does not bypass: PostgreSQL refuses the
    // query instead (the role has no BYPASSRLS).
    let err = gateway
        .run(&a, |conn| {
            let table = f.table("journal");
            Box::pin(async move {
                sqlx::query("set local row_security = off")
                    .execute(&mut *conn)
                    .await
                    .map_err(BackendError::unknown)?;
                sqlx::query_scalar::<_, i64>(&format!("select count(*) from {table}"))
                    .fetch_one(conn)
                    .await
                    .map_err(BackendError::unknown)
            })
        })
        .await
        .unwrap_err();
    assert!(
        message(&err).contains("row-level security"),
        "{}",
        message(&err)
    );

    // A login that was not granted the system role cannot become it,
    // neither through the entry point nor by hand.
    for mode_access in [
        gateway.policy().system_access(),
        gateway.policy().inspection_access(),
    ] {
        let err = gateway.begin(&mode_access).await.unwrap_err();
        assert!(
            message(&err).contains("permission denied"),
            "{}",
            message(&err)
        );
    }
    let pool = gateway.pool();
    let err = sqlx::query(&format!("SET ROLE {PREFIX}_system"))
        .execute(pool)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("permission denied"), "{err}");
    f.cleanup().await;
}

// ---------------------------------------------------------------------------
// Inspection and system modes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn inspection_reads_every_tenant_and_every_write_is_denied() {
    let f = Fixture::new("inspection").await;
    let web = RlsGateway::new(login_pool(WEB, 4).await, policy());
    seed(&f, &web, "a").await;
    seed(&f, &web, "b").await;
    let agent = RlsGateway::new(login_pool(AGENT, 4).await, policy());
    let inspection = agent.policy().inspection_access();

    for table in TABLES {
        assert_eq!(
            count(&agent, &inspection, f.table(table)).await,
            2,
            "{table}"
        );
    }
    for table in TABLES {
        let (sql, bind) = insert_sql(&f, table, "a/s7");
        let err = exec(&agent, &inspection, sql, bind).await.unwrap_err();
        assert!(
            message(&err).contains("permission denied"),
            "{table}: {}",
            message(&err)
        );
        let t = f.table(table);
        for sql in [
            format!("update {t} set payload = '{{}}'::jsonb"),
            format!("delete from {t}"),
        ] {
            // `payload` exists on journal/outbox only: any failure is a denial.
            assert!(
                exec(&agent, &inspection, sql, vec![]).await.is_err(),
                "{table}"
            );
        }
    }
    // The agent's login cannot act as a tenant.
    let err = agent.begin(&tenant(agent.policy(), "a")).await.unwrap_err();
    assert!(
        message(&err).contains("permission denied"),
        "{}",
        message(&err)
    );
    f.cleanup().await;
}

#[tokio::test]
async fn system_has_full_access_only_through_the_entry_point() {
    let f = Fixture::new("system").await;
    let web = RlsGateway::new(login_pool(WEB, 4).await, policy());
    seed(&f, &web, "a").await;
    seed(&f, &web, "b").await;
    let job = RlsGateway::new(login_pool(JOB, 4).await, policy());
    let system = job.policy().system_access();

    for table in TABLES {
        assert_eq!(count(&job, &system, f.table(table)).await, 2, "{table}");
    }
    // Writes for any tenant, updates and deletes included.
    for table in TABLES {
        let (sql, bind) = insert_sql(&f, table, "c/s1");
        assert_eq!(exec(&job, &system, sql, bind).await.unwrap(), 1, "{table}");
        assert_eq!(count(&job, &system, f.table(table)).await, 3);
        assert_eq!(
            exec(
                &job,
                &system,
                format!("delete from {} where tenant_id = 'c'", f.table(table)),
                vec![]
            )
            .await
            .unwrap(),
            1
        );
    }

    // Without the entry point the system login has no access at all: it
    // holds the system role by membership, but nothing is granted to the
    // login itself (NOINHERIT) and no policy covers it.
    let err = sqlx::query(&format!("select count(*) from {}", f.table("journal")))
        .fetch_one(job.pool())
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("permission denied"), "{err}");
    f.cleanup().await;
}

// ---------------------------------------------------------------------------
// The catalog: FORCE, ownership, policies
// ---------------------------------------------------------------------------

#[tokio::test]
async fn row_level_security_is_forced_and_the_owner_does_not_bypass_it() {
    let f = Fixture::new("force").await;
    let web = RlsGateway::new(login_pool(WEB, 4).await, policy());
    seed(&f, &web, "a").await;

    let flags: Vec<(String, bool, bool)> = sqlx::query_as(
        "select c.relname::text, c.relrowsecurity, c.relforcerowsecurity from pg_class c join pg_namespace n on n.oid = c.relnamespace where n.nspname = $1 and c.relkind = 'r' order by 1",
    )
    .bind(&f.schema)
    .fetch_all(&f.admin)
    .await
    .unwrap();
    assert_eq!(
        flags,
        TABLES
            .iter()
            .map(|t| (t.to_string(), true, true))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
    );

    // The owner is a plain role covered by no policy: with FORCE it sees
    // nothing in its own tables; without FORCE it would see everything.
    let mut conn = f.admin.acquire().await.unwrap();
    sqlx::query(&format!("SET ROLE {OWNER}"))
        .execute(&mut *conn)
        .await
        .unwrap();
    for table in TABLES {
        let n: i64 = sqlx::query_scalar(&format!("select count(*) from {}", f.table(table)))
            .fetch_one(&mut *conn)
            .await
            .unwrap();
        assert_eq!(n, 0, "the owner must not bypass RLS on {table}");
    }
    sqlx::query("RESET ROLE").execute(&mut *conn).await.unwrap();

    // The tables belong to the owner, never to an application role, and
    // none of the roles can bypass RLS.
    let owners: Vec<String> =
        sqlx::query_scalar("select distinct tableowner::text from pg_tables where schemaname = $1")
            .bind(&f.schema)
            .fetch_all(&f.admin)
            .await
            .unwrap();
    assert_eq!(owners, vec![OWNER.to_string()]);
    let powerful: Vec<String> = sqlx::query_scalar(
        "select rolname::text from pg_roles where rolname = any($1) and (rolsuper or rolbypassrls or rolcreaterole)",
    )
    .bind(vec![
        policy().tenant_role().to_string(),
        policy().system_role().to_string(),
        policy().inspection_role().to_string(),
        WEB.to_string(),
        JOB.to_string(),
        AGENT.to_string(),
        APP.to_string(),
    ])
    .fetch_all(&f.admin)
    .await
    .unwrap();
    assert!(powerful.is_empty(), "{powerful:?}");
    f.cleanup().await;
}

#[tokio::test]
async fn every_table_has_the_three_policies_with_using_and_with_check() {
    let f = Fixture::new("policies").await;
    #[allow(clippy::type_complexity)]
    let rows: Vec<(String, String, String, Vec<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "select tablename::text, policyname::text, cmd::text, roles::text[], qual, with_check from pg_policies where schemaname = $1 order by 1, 2",
    )
    .bind(&f.schema)
    .fetch_all(&f.admin)
    .await
    .unwrap();
    assert_eq!(rows.len(), 12);
    for (table, name, cmd, roles, qual, with_check) in rows {
        if name.ends_with("_tenant_policy") {
            assert_eq!(
                (cmd.as_str(), roles),
                ("ALL", vec![format!("{PREFIX}_tenant")]),
                "{table}"
            );
            let qual = qual.expect("USING");
            let check = with_check.expect("WITH CHECK");
            for expr in [&qual, &check] {
                assert!(expr.contains("tenant_id"), "{table}: {expr}");
                assert!(expr.contains("current_setting"), "{table}: {expr}");
                assert_ne!(
                    expr.trim_matches(|c| c == '(' || c == ')'),
                    "true",
                    "{table}"
                );
            }
        } else if name.ends_with("_inspection_policy") {
            assert_eq!(cmd, "SELECT", "{table}");
            assert_eq!(roles, vec![format!("{PREFIX}_inspection")]);
            assert!(with_check.is_none());
        } else {
            assert!(name.ends_with("_system_policy"), "{name}");
            assert_eq!(
                (cmd.as_str(), roles),
                ("ALL", vec![format!("{PREFIX}_system")]),
                "{table}"
            );
        }
    }
    f.cleanup().await;
}

// ---------------------------------------------------------------------------
// The guarded SaaS service, end to end
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Note {
    text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum NoteEvent {
    Created {
        tenant: String,
        owner: String,
        text: String,
    },
    Updated(String),
}

type State = CrudState<Note>;
type Service = SaaSEventSourcedService<CallerIdentity, String, Note, NoteEvent, String, String>;
type App = SaaSEsApp<CallerIdentity, String, Note, NoteEvent, String, String, ()>;

struct NoteModel;

impl DomainModel for NoteModel {
    type State = State;
    type Event = NoteEvent;
    type Rejection = String;

    fn initial(&self) -> State {
        CrudState::NonExistent
    }

    fn transition(&self, e: &NoteEvent, s: State) -> Result<State, NonEmpty<String>> {
        match (e, s) {
            (
                NoteEvent::Created {
                    tenant,
                    owner,
                    text,
                },
                _,
            ) => Ok(CrudState::active(
                tenant.as_str(),
                owner.as_str(),
                Note { text: text.clone() },
            )),
            (
                NoteEvent::Updated(text),
                CrudState::Active {
                    tenant_id,
                    owner_id,
                    ..
                },
            ) => Ok(CrudState::Active {
                tenant_id,
                owner_id,
                data: Note { text: text.clone() },
            }),
            (NoteEvent::Updated(_), _) => Err(NonEmpty::new("not active".to_string())),
        }
    }
}

fn service() -> Service {
    SaaSEventSourcedService::builder()
        .auth_policy(PermissivePolicy)
        .rejection(|m| m)
        .access_policy(policy())
        .build()
}

fn create_event(caller: &CallerIdentity, text: &str) -> NoteEvent {
    NoteEvent::Created {
        tenant: caller.tenant_id.value().to_string(),
        owner: caller.user_id.value().to_string(),
        text: text.to_string(),
    }
}

/// `create:<text>` / `update:<text>`; checked by the application guard.
fn guarded_app(service: &Service) -> App {
    let saas = service.saas().clone();
    saas.clone().guarded_router(move |c| {
        let s = saas.clone();
        match c.split_once(':') {
            Some(("create", text)) => {
                let text = text.to_string();
                let s2 = s.clone();
                (
                    CrudAction::Create,
                    s.auth()
                        .and_then(move |caller| {
                            s2.decide(Decision::accept(create_event(&caller, &text)))
                        })
                        .then(s.publish([format!("created {c}")])),
                )
            }
            Some(("update", text)) => (
                CrudAction::Update,
                s.accept(NoteEvent::Updated(text.to_string())),
            ),
            _ => (CrudAction::Read, s.unit()),
        }
    })
}

/// The same programs with **no** application check: the guard is bypassed,
/// as in a bug or an `unsafe_*` use; only the database stands.
fn unguarded_app(service: &Service) -> App {
    let saas = service.saas().clone();
    saas.clone().unsafe_unguarded_router(move |c| {
        let s = saas.clone();
        match c.split_once(':') {
            Some(("create", text)) => {
                let text = text.to_string();
                let s2 = s.clone();
                s.auth().and_then(move |caller| {
                    s2.decide(Decision::accept(create_event(&caller, &text)))
                })
            }
            Some(("update", text)) => s.accept(NoteEvent::Updated(text.to_string())),
            _ => s.unit(),
        }
    })
}

fn command(
    id: &str,
    caller_tenant: &str,
    address: &str,
    payload: &str,
) -> edomata_saas::CommandMessage<SaaSCommand<CallerIdentity, String>> {
    edomata_saas::CommandMessage::new(
        id,
        chrono::Utc::now(),
        address,
        SaaSCommand::new(
            CallerIdentity::new(
                caller_tenant,
                format!("user-of-{caller_tenant}"),
                Vec::<String>::new(),
            ),
            payload.to_string(),
        ),
    )
}

/// A snapshot store that keeps nothing: the in-process caches are not
/// tenant-scoped, so a test of what the *database* refuses must not be
/// answered from memory.
struct NoSnapshots;

#[async_trait]
impl SnapshotReader<State> for NoSnapshots {
    async fn get(&self, _: &str) -> Result<Option<ValidState<State>>, BackendError> {
        Ok(None)
    }
    async fn get_fast(&self, _: &str) -> Option<ValidState<State>> {
        None
    }
}

#[async_trait]
impl SnapshotStore<State> for NoSnapshots {
    async fn put(&self, _: &str, _: ValidState<State>) -> Result<(), BackendError> {
        Ok(())
    }
}

type NoteBackend = Backend<State, NoteEvent, String, String>;

async fn backend(service: &Service, gateway: &RlsGateway, naming: PGNaming) -> NoteBackend {
    let driver = gateway.driver(naming).await.unwrap();
    Backend::builder(NoteModel, service.domain())
        .driver(driver)
        .disable_cache()
        .with_snapshot(Arc::new(NoSnapshots))
        .build_default()
        .await
        .unwrap()
}

#[tokio::test]
async fn the_guarded_service_runs_as_the_tenant_role_through_the_entry_point() {
    let f = Fixture::new("e2e_service").await;
    let service = service();
    let gateway = RlsGateway::new(login_pool(WEB, 4).await, service.access_policy().clone());
    let backend = backend(&service, &gateway, f.naming()).await;
    let run = backend.compile(guarded_app(&service));
    let a = tenant(gateway.policy(), "acme");
    let id = |x: &str| {
        gateway
            .policy()
            .scoped_id(&TenantId::new("acme"), x)
            .unwrap()
    };

    // The web login owns nothing and may only become the tenant role: this
    // succeeding proves the whole command ran as that role.
    gateway
        .with_access(
            &a,
            run(command(&id("c1"), "acme", &id("n1"), "create:hello")),
        )
        .await
        .unwrap()
        .unwrap();
    gateway
        .with_access(
            &a,
            run(command(&id("c2"), "acme", &id("n1"), "update:world")),
        )
        .await
        .unwrap()
        .unwrap();

    let state = gateway
        .with_access(&a, backend.repository().get(&id("n1")))
        .await
        .unwrap();
    let AggregateState::Valid(state) = state else {
        panic!("conflicted")
    };
    assert_eq!(state.version, 2);
    assert_eq!(
        state.state.data(),
        Some(&Note {
            text: "world".into()
        })
    );

    // What the generated column recorded, seen by the superuser.
    for table in ["journal", "outbox", "commands"] {
        let tenants: Vec<String> = sqlx::query_scalar(&format!(
            "select distinct tenant_id from {}",
            f.table(table)
        ))
        .fetch_all(&f.admin)
        .await
        .unwrap();
        assert_eq!(tenants, vec!["acme".to_string()], "{table}");
    }
    f.cleanup().await;
}

#[tokio::test]
async fn tenant_b_reading_or_appending_on_tenant_as_stream_is_refused_by_the_database() {
    let f = Fixture::new("e2e_cross_tenant").await;
    let service = service();
    let gateway = RlsGateway::new(login_pool(WEB, 4).await, service.access_policy().clone());
    let backend = backend(&service, &gateway, f.naming()).await;
    let guarded = backend.compile(guarded_app(&service));
    let unguarded = backend.compile(unguarded_app(&service));
    let a = tenant(gateway.policy(), "a");
    let b = tenant(gateway.policy(), "b");

    gateway
        .with_access(&a, guarded(command("a/c1", "a", "a/n1", "create:secret")))
        .await
        .unwrap()
        .unwrap();

    // READ: B sees nothing of A's stream, A sees its event.
    let read = |access: &Access| {
        let journal = Arc::clone(backend.journal());
        let access = access.clone();
        let gateway = gateway.clone();
        async move {
            use futures::TryStreamExt;
            gateway
                .with_access(&access, async move {
                    journal.read_stream("a/n1").try_collect::<Vec<_>>().await
                })
                .await
                .unwrap()
        }
    };
    assert_eq!(read(&a).await.len(), 1);
    assert!(
        read(&b).await.is_empty(),
        "the database hides A's events from B"
    );
    let seen = gateway
        .with_access(&b, backend.repository().get("a/n1"))
        .await
        .unwrap();
    let AggregateState::Valid(seen) = seen else {
        panic!("conflicted")
    };
    assert_eq!(seen.version, 0, "B sees no state for A's stream");

    // APPEND with the application guard BYPASSED: B claims to be B and
    // creates/updates on A's stream. The journal insert fails the WITH CHECK.
    // A create is refused by the journal insert; an update never gets
    // that far: A's events are invisible to B, so for B the entity does
    // not exist and the model refuses the transition.
    let err = gateway
        .with_access(&b, unguarded(command("b/c1", "b", "a/n1", "create:hijack")))
        .await
        .expect_err("create:hijack");
    assert!(
        message(&err).contains("row-level security"),
        "{}",
        message(&err)
    );
    let rejected = gateway
        .with_access(&b, unguarded(command("b/c0", "b", "a/n1", "update:hijack")))
        .await
        .unwrap();
    assert_eq!(
        rejected.map_err(|r| r.to_vec()),
        Err(vec!["not active".to_string()])
    );
    // ... even claiming A's identity in the command (the guard would pass):
    let err = gateway
        .with_access(&b, unguarded(command("b/c2", "a", "a/n1", "create:forged")))
        .await
        .unwrap_err();
    assert!(
        message(&err).contains("row-level security"),
        "{}",
        message(&err)
    );
    // ... and replaying one of A's command ids is refused too.
    let err = gateway
        .with_access(&b, unguarded(command("a/c1", "b", "b/n1", "create:replay")))
        .await
        .unwrap_err();
    assert!(
        message(&err).contains("row-level security"),
        "{}",
        message(&err)
    );

    // The guarded path rejects as well (the row is invisible, so the entity
    // does not exist for B): two layers, the database does not need the first.
    let rejected = gateway
        .with_access(&b, guarded(command("b/c3", "b", "a/n1", "update:x")))
        .await
        .unwrap();
    assert_eq!(
        rejected.map_err(|r| r.to_vec()),
        Err(vec!["Entity not found".to_string()])
    );

    // Nothing of B reached A's data.
    let rows: Vec<(String, i64)> = sqlx::query_as(&format!(
        "select tenant_id, count(*) from {} group by 1",
        f.table("journal")
    ))
    .fetch_all(&f.admin)
    .await
    .unwrap();
    assert_eq!(rows, vec![("a".to_string(), 1)]);
    f.cleanup().await;
}

#[tokio::test]
async fn a_driver_used_outside_the_entry_point_fails_closed() {
    let f = Fixture::new("e2e_no_context").await;
    let service = service();
    let gateway = RlsGateway::new(login_pool(WEB, 2).await, service.access_policy().clone());
    let backend = backend(&service, &gateway, f.naming()).await;
    let run = backend.compile(guarded_app(&service));
    let err = run(command("a/c1", "a", "a/n1", "create:x"))
        .await
        .unwrap_err();
    assert!(
        message(&err).contains("no access context"),
        "{}",
        message(&err)
    );
    f.cleanup().await;
}

#[tokio::test]
async fn a_relay_in_system_mode_reads_every_tenants_outbox_a_tenant_only_its_own() {
    use futures::TryStreamExt;

    let f = Fixture::new("e2e_relay").await;
    let service = service();
    let web = RlsGateway::new(login_pool(WEB, 4).await, service.access_policy().clone());
    let web_backend = backend(&service, &web, f.naming()).await;
    let run = web_backend.compile(guarded_app(&service));
    for t in ["a", "b"] {
        web.with_access(
            &tenant(web.policy(), t),
            run(command(
                &format!("{t}/c1"),
                t,
                &format!("{t}/n1"),
                "create:x",
            )),
        )
        .await
        .unwrap()
        .unwrap();
    }

    let job = RlsGateway::new(login_pool(JOB, 4).await, service.access_policy().clone());
    let job_backend = backend(&service, &job, f.naming()).await;
    let system = job.policy().system_access();
    let outbox = Arc::clone(job_backend.outbox());
    let items = job
        .with_access(&system, async move {
            outbox.read().try_collect::<Vec<_>>().await
        })
        .await
        .unwrap();
    let mut streams: Vec<String> = items.iter().map(|i| i.stream_id.clone()).collect();
    streams.sort();
    assert_eq!(streams, vec!["a/n1".to_string(), "b/n1".to_string()]);
    // Marking as sent is a system write.
    let outbox = Arc::clone(job_backend.outbox());
    let items = NonEmpty::from_vec(items).unwrap();
    job.with_access(
        &system,
        async move { outbox.mark_all_as_sent(&items).await },
    )
    .await
    .unwrap();

    // A tenant's relay-less view: its own (already sent) rows only.
    let outbox = Arc::clone(web_backend.outbox());
    let unsent = web
        .with_access(&tenant(web.policy(), "a"), async move {
            outbox.read().try_collect::<Vec<_>>().await
        })
        .await
        .unwrap();
    assert!(unsent.is_empty());
    let total: i64 = sqlx::query_scalar(&format!("select count(*) from {}", f.table("outbox")))
        .fetch_one(&f.admin)
        .await
        .unwrap();
    assert_eq!(total, 2);
    f.cleanup().await;
}

#[tokio::test]
async fn persisted_snapshots_are_written_by_an_explicit_system_scope() {
    let f = Fixture::new("e2e_snapshots").await;
    let service = service();
    // A process that may be tenant for requests and system for the
    // background snapshot writes.
    let gateway = RlsGateway::new(login_pool(APP, 4).await, service.access_policy().clone());
    let driver = gateway
        .driver(f.naming())
        .await
        .unwrap()
        .with_snapshot_write_scope(gateway.fixed_scope(gateway.policy().system_access()));
    let backend: NoteBackend = Backend::builder(NoteModel, service.domain())
        .driver(driver)
        .persisted_snapshot(SqlxCodec::<State>::jsonb())
        .build_default()
        .await
        .unwrap();
    let run = backend.compile(guarded_app(&service));
    let a = tenant(gateway.policy(), "a");
    gateway
        .with_access(&a, run(command("a/c1", "a", "a/n1", "create:x")))
        .await
        .unwrap()
        .unwrap();
    backend.close().await.unwrap();

    let rows: Vec<(String, String)> = sqlx::query_as(&format!(
        "select id, tenant_id from {}",
        f.table("snapshots")
    ))
    .fetch_all(&f.admin)
    .await
    .unwrap();
    assert_eq!(rows, vec![("a/n1".to_string(), "a".to_string())]);
    f.cleanup().await;
}
