//! PostgreSQL integration tests for the tenant-aware CQRS driver (the Scala
//! `saas-skunk` module has no test suite of its own; these pin the column
//! population, the DDL executed against a live server, RLS enforcement,
//! the transactional handler and `TenantStateLister`).

use edomata_backend::BackendError;
use edomata_backend::cqrs::Backend;
use edomata_core::{CommandMessage, CqrsModel};
use edomata_saas::{
    CallerIdentity, CrudAction, CrudState, PGNaming, PermissivePolicy, RlsConfig, SaaSCommand,
    SaaSCqrsApp, SaaSCqrsService, SaaSPGSchema, TenantId,
};
use edomata_saas_sqlx::{SaaSCodec, SaaSSqlxCqrsDriver, SqlxHandler, TenantStateLister};
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    PgPoolOptions::new()
        .max_connections(8)
        .connect(&url)
        .await
        .expect("PostgreSQL from docker-compose must be running (see rust/README.md)")
}

async fn drop_prefixed(pool: &PgPool, prefix: &str) {
    for t in ["states", "outbox", "commands"] {
        sqlx::query(&format!("DROP TABLE IF EXISTS {prefix}_{t}"))
            .execute(pool)
            .await
            .unwrap();
    }
}

async fn drop_schema(pool: &PgPool, ns: &str) {
    sqlx::query(&format!("DROP SCHEMA IF EXISTS \"{ns}\" CASCADE"))
        .execute(pool)
        .await
        .unwrap();
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Note {
    text: String,
}

type State = CrudState<Note>;
type Service = SaaSCqrsService<CallerIdentity, String, Note, String, String>;
type App = SaaSCqrsApp<CallerIdentity, String, Note, String, String, ()>;

struct NoteModel;

impl CqrsModel for NoteModel {
    type State = State;
    type Rejection = String;

    fn initial(&self) -> State {
        CrudState::NonExistent
    }
}

fn service() -> Service {
    SaaSCqrsService::new(PermissivePolicy, |m| m)
}

/// `create:<text>` creates, `update:<text>` updates, `ping` publishes without
/// changing the state, `fail` rejects while publishing.
fn app(service: &Service) -> App {
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
                            s2.set(CrudState::active(
                                caller.tenant_id,
                                caller.user_id,
                                Note { text: text.clone() },
                            ))
                        })
                        .then(s.publish(["created".to_string()])),
                )
            }
            Some(("update", text)) => {
                let text = text.to_string();
                (
                    CrudAction::Update,
                    s.modify_s(move |st| Ok(st.map(|_| Note { text: text.clone() })))
                        .void()
                        .then(s.publish(["updated".to_string()])),
                )
            }
            _ if c == "ping" => (CrudAction::Read, s.publish(["pong".to_string()])),
            _ if c == "fail" => (
                CrudAction::Read,
                s.publish(["failing".to_string()])
                    .then(s.reject("nope".to_string())),
            ),
            _ => (CrudAction::Read, s.unit()),
        }
    })
}

fn cmd(
    id: &str,
    tenant: &str,
    address: &str,
    payload: &str,
) -> CommandMessage<SaaSCommand<CallerIdentity, String>> {
    CommandMessage::new(
        id,
        chrono::Utc::now(),
        address,
        SaaSCommand::new(
            CallerIdentity::new(tenant, format!("user-of-{tenant}"), Vec::<String>::new()),
            payload.to_string(),
        ),
    )
}

async fn backend(
    driver: SaaSSqlxCqrsDriver,
    handler: Option<SqlxHandler<String>>,
) -> Backend<State, String> {
    let service = service();
    let mut builder = Backend::builder(NoteModel, service.domain()).driver(driver);
    if let Some(h) = handler {
        builder = builder.with_event_handler(h);
    }
    builder
        .build(SaaSCodec::jsonb_state(), SaaSCodec::jsonb_notification())
        .await
        .unwrap()
}

#[tokio::test]
async fn save_populates_tenant_and_owner_columns() {
    let pool = pool().await;
    drop_prefixed(&pool, "saas_sqlx_cols").await;
    let driver = SaaSSqlxCqrsDriver::new(
        PGNaming::prefixed_str("saas_sqlx_cols").unwrap(),
        pool.clone(),
    )
    .await
    .unwrap();
    let backend = backend(driver.clone(), None).await;
    let service = service();
    let run = backend.compile(app(&service));

    run(cmd("c1", "tenant-a", "n1", "create:hello"))
        .await
        .unwrap()
        .unwrap();
    run(cmd("c2", "tenant-b", "n2", "create:world"))
        .await
        .unwrap()
        .unwrap();
    run(cmd("c3", "tenant-a", "n1", "update:hello again"))
        .await
        .unwrap()
        .unwrap();

    let rows: Vec<(String, String, String, i64)> = sqlx::query_as(
        "select id, tenant_id, owner_id, version from saas_sqlx_cols_states order by id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![
            ("n1".into(), "tenant-a".into(), "user-of-tenant-a".into(), 2),
            ("n2".into(), "tenant-b".into(), "user-of-tenant-b".into(), 1),
        ]
    );
    let outbox: Vec<(String, String)> =
        sqlx::query_as("select stream, tenant_id from saas_sqlx_cols_outbox order by seqnr")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        outbox,
        vec![
            ("n1".into(), "tenant-a".into()),
            ("n2".into(), "tenant-b".into()),
            ("n1".into(), "tenant-a".into()),
        ]
    );

    // Reads go through the standard repository reader.
    let state = backend.repository().get("n1").await.unwrap();
    assert_eq!(state.version, 2);
    assert_eq!(
        state.state.data(),
        Some(&Note {
            text: "hello again".into()
        })
    );
    let missing = backend.repository().get("nope").await.unwrap();
    assert_eq!(missing.state, CrudState::NonExistent);
    assert_eq!(missing.version, 0);

    // Redundant commands are ignored.
    run(cmd("c1", "tenant-a", "n1", "update:ignored"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(backend.repository().get("n1").await.unwrap().version, 2);

    // TenantStateLister scopes by tenant.
    let lister = TenantStateLister::new(&driver, SaaSCodec::<State>::jsonb_state());
    let a = lister
        .list_by_tenant(&TenantId::new("tenant-a"))
        .await
        .unwrap();
    assert_eq!(a.len(), 1);
    assert_eq!(
        a[0].state.data(),
        Some(&Note {
            text: "hello again".into()
        })
    );
    let b = lister
        .list_by_tenant(&TenantId::new("tenant-b"))
        .await
        .unwrap();
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].state.tenant_id(), Some(&TenantId::new("tenant-b")));
    assert!(
        lister
            .list_by_tenant(&TenantId::new("tenant-c"))
            .await
            .unwrap()
            .is_empty()
    );

    // The outbox reader sees every item regardless of tenant.
    let items: Vec<_> = backend.outbox().read().try_collect().await.unwrap();
    assert_eq!(items.len(), 3);
}

#[tokio::test]
async fn notify_without_state_change_and_rejections_use_an_empty_tenant() {
    let pool = pool().await;
    drop_schema(&pool, "saas_sqlx_notify").await;
    let driver = SaaSSqlxCqrsDriver::for_namespace("saas_sqlx_notify", pool.clone())
        .await
        .unwrap();
    let backend = backend(driver, None).await;
    let service = service();
    let run = backend.compile(app(&service));

    run(cmd("c1", "tenant-a", "n1", "create:x"))
        .await
        .unwrap()
        .unwrap();
    // A read-only program that publishes: the state is saved again (as in
    // Scala's CQRS handler), so the tenant is known.
    run(cmd("c2", "tenant-a", "n1", "ping"))
        .await
        .unwrap()
        .unwrap();
    // A rejection that publishes goes through `notify` without a state, so
    // the tenant is empty (Scala behaviour).
    let rejected = run(cmd("c3", "tenant-a", "n1", "fail")).await.unwrap();
    assert!(rejected.is_err());

    let outbox: Vec<(String, String)> = sqlx::query_as(
        "select payload::text, tenant_id from \"saas_sqlx_notify\".outbox order by seqnr",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        outbox,
        vec![
            ("\"created\"".into(), "tenant-a".into()),
            ("\"pong\"".into(), "tenant-a".into()),
            ("\"failing\"".into(), String::new()),
        ]
    );
    assert_eq!(backend.repository().get("n1").await.unwrap().version, 2);
}

#[tokio::test]
async fn setup_creates_the_saas_catalog_objects() {
    let pool = pool().await;
    drop_prefixed(&pool, "saas_sqlx_ddl").await;
    let driver = SaaSSqlxCqrsDriver::new(
        PGNaming::prefixed_str("saas_sqlx_ddl").unwrap(),
        pool.clone(),
    )
    .await
    .unwrap();
    assert!(driver.auto_setup());
    let _backend = backend(driver, None).await;

    let columns: Vec<(String, String)> = sqlx::query_as(
        "select table_name::text, column_name::text from information_schema.columns where table_name in ('saas_sqlx_ddl_states', 'saas_sqlx_ddl_outbox', 'saas_sqlx_ddl_commands') and column_name in ('tenant_id', 'owner_id') order by 1, 2",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        columns,
        vec![
            ("saas_sqlx_ddl_outbox".into(), "tenant_id".into()),
            ("saas_sqlx_ddl_states".into(), "owner_id".into()),
            ("saas_sqlx_ddl_states".into(), "tenant_id".into()),
        ]
    );
    let indexes: Vec<String> = sqlx::query_scalar(
        "select indexname::text from pg_indexes where indexname like 'saas_sqlx_ddl_%_idx' order by 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        indexes,
        vec![
            "saas_sqlx_ddl_outbox_tenant_idx".to_string(),
            "saas_sqlx_ddl_states_tenant_idx".to_string(),
            "saas_sqlx_ddl_states_tenant_owner_idx".to_string(),
        ]
    );
}

#[tokio::test]
async fn skip_setup_with_rls_ddl_from_saas_pgschema_enforces_tenant_isolation() {
    let pool = pool().await;
    drop_prefixed(&pool, "saas_sqlx_rls").await;
    let role = "edomata_saas_rls_test";
    sqlx::query(&format!(
        "do $$ begin if not exists (select 1 from pg_roles where rolname = '{role}') then create role {role} nologin; end if; end $$"
    ))
    .execute(&pool)
    .await
    .unwrap();

    // The Flyway workflow: DDL (with RLS) from SaaSPGSchema, then skip_setup.
    let naming = PGNaming::prefixed_str("saas_sqlx_rls").unwrap();
    let rls = RlsConfig::new(role, "app.tenant_id");
    for statement in SaaSPGSchema::cqrs_with(&naming, "jsonb", "jsonb", Some(&rls)) {
        sqlx::query(&statement).execute(&pool).await.unwrap();
    }
    let driver = SaaSSqlxCqrsDriver::new_with(naming, pool.clone(), true)
        .await
        .unwrap();
    assert!(!driver.auto_setup());
    let backend = backend(driver, None).await;
    let service = service();
    let run = backend.compile(app(&service));
    run(cmd("c1", "tenant-a", "r1", "create:a"))
        .await
        .unwrap()
        .unwrap();
    run(cmd("c2", "tenant-b", "r2", "create:b"))
        .await
        .unwrap()
        .unwrap();
    run(cmd("c3", "tenant-b", "r3", "create:b2"))
        .await
        .unwrap()
        .unwrap();

    let policies: Vec<String> = sqlx::query_scalar(
        "select policyname::text from pg_policies where tablename like 'saas_sqlx_rls_%' order by 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        policies,
        vec![
            "saas_sqlx_rls_outbox_tenant_policy".to_string(),
            "saas_sqlx_rls_states_tenant_policy".to_string(),
        ]
    );

    // The superuser bypasses RLS; the application role only sees its tenant.
    let mut conn = pool.acquire().await.unwrap();
    sqlx::query(&format!("set role {role}"))
        .execute(&mut *conn)
        .await
        .unwrap();
    sqlx::query("select set_config('app.tenant_id', 'tenant-b', false)")
        .execute(&mut *conn)
        .await
        .unwrap();
    let states: i64 = sqlx::query_scalar("select count(*) from saas_sqlx_rls_states")
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    let outbox: i64 = sqlx::query_scalar("select count(*) from saas_sqlx_rls_outbox")
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    assert_eq!((states, outbox), (2, 2));
    sqlx::query("select set_config('app.tenant_id', 'tenant-a', false)")
        .execute(&mut *conn)
        .await
        .unwrap();
    let states: i64 = sqlx::query_scalar("select count(*) from saas_sqlx_rls_states")
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    assert_eq!(states, 1);
    sqlx::query("reset role").execute(&mut *conn).await.unwrap();
    drop(conn);
}

#[tokio::test]
async fn handler_runs_inside_the_save_transaction() {
    let pool = pool().await;
    drop_schema(&pool, "saas_sqlx_handler").await;
    sqlx::query("create schema saas_sqlx_handler")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("create table saas_sqlx_handler.projection (value text not null)")
        .execute(&pool)
        .await
        .unwrap();
    let driver = SaaSSqlxCqrsDriver::new(
        PGNaming::schema_str("saas_sqlx_handler").unwrap(),
        pool.clone(),
    )
    .await
    .unwrap();
    let handler: SqlxHandler<String> = SqlxHandler::new(|ns, conn| {
        Box::pin(async move {
            for n in ns.iter() {
                if n == "updated" {
                    return Err(BackendError::persistence("projection refused the update"));
                }
                sqlx::query("insert into saas_sqlx_handler.projection(value) values ($1)")
                    .bind(n)
                    .execute(&mut *conn)
                    .await
                    .map_err(BackendError::unknown)?;
            }
            Ok(())
        })
    });
    let backend = backend(driver, Some(handler)).await;
    let service = service();
    let run = backend.compile(app(&service));

    run(cmd("c1", "tenant-a", "h1", "create:x"))
        .await
        .unwrap()
        .unwrap();
    let err = run(cmd("c2", "tenant-a", "h1", "update:y"))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("projection refused"), "{err}");

    // The failed save was rolled back entirely: state, outbox and command.
    assert_eq!(backend.repository().get("h1").await.unwrap().version, 1);
    let outbox: i64 = sqlx::query_scalar("select count(*) from \"saas_sqlx_handler\".outbox")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(outbox, 1);
    let projected: Vec<String> =
        sqlx::query_scalar("select value from saas_sqlx_handler.projection")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(projected, vec!["created".to_string()]);
}

#[tokio::test]
async fn for_namespace_validates_the_name() {
    let err = SaaSSqlxCqrsDriver::for_namespace("1-bad", pool().await)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("does not match"), "{err}");
}

#[tokio::test]
async fn saas_codec_exposes_format_and_extractor() {
    let state = SaaSCodec::<State>::jsonb_state();
    assert_eq!(state.sql_type(), "jsonb");
    assert_eq!(
        state.tenant_and_owner(&CrudState::active("t", "u", Note { text: "x".into() })),
        Some((TenantId::new("t"), edomata_saas::UserId::new("u")))
    );
    assert_eq!(state.tenant_and_owner(&CrudState::NonExistent), None);
    let notif = SaaSCodec::<String>::jsonb_notification();
    assert_eq!(notif.tenant_and_owner(&"n".to_string()), None);
    assert!(format!("{state:?}").contains("tenant_aware: true"));
    let custom =
        SaaSCodec::<String>::with_extractor(edomata_serde::SerdeCodec::<String>::json(), |s| {
            Some((TenantId::new(s.clone()), edomata_saas::UserId::new("owner")))
        });
    assert_eq!(custom.sql_type(), "json");
    assert_eq!(
        custom.tenant_and_owner(&"acme".to_string()),
        Some((TenantId::new("acme"), edomata_saas::UserId::new("owner")))
    );
}
