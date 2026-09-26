//! Port of `examples/src/main/scala/SaaSExample.scala`: a multi-tenant todo
//! service (CQRS) with automatic tenant and role guards, an admin bypass, a
//! SQL read projection maintained by the notification handler, and
//! tenant-scoped read queries.
//!
//! ```sh
//! cargo run -p edomata-examples --bin saas_todo
//! ```

use std::collections::BTreeSet;

use edomata_backend::BackendError;
use edomata_backend::cqrs::Backend;
use edomata_core::CqrsModel;
use edomata_examples::{command, connect};
use edomata_saas::*;
use edomata_sqlx::{PgPool, SqlxCqrsDriver, SqlxHandler};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// 1. DOMAIN TYPES
// ---------------------------------------------------------------------------

/// Business entity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Todo {
    title: String,
    completed: bool,
}

/// Business commands.
#[derive(Clone, Debug, PartialEq, Eq)]
enum TodoCommand {
    Create(String),
    Complete,
    Delete,
}

/// Notifications emitted to the outbox (integration events).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all_fields = "camelCase")]
enum TodoNotification {
    Created {
        entity_id: String,
        tenant_id: String,
        title: String,
    },
    Completed {
        entity_id: String,
        tenant_id: String,
    },
    Deleted {
        entity_id: String,
        tenant_id: String,
    },
}

// ---------------------------------------------------------------------------
// 2. CQRS MODEL (state = CrudState<Todo>) and role-based policy
// ---------------------------------------------------------------------------

struct TodoModel;

impl CqrsModel for TodoModel {
    type State = CrudState<Todo>;
    type Rejection = String;

    fn initial(&self) -> CrudState<Todo> {
        CrudState::NonExistent
    }
}

fn roles(xs: &[&str]) -> BTreeSet<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

fn policy() -> RoleBasedPolicy {
    RoleBasedPolicy::new(|action| match action {
        CrudAction::Create | CrudAction::Update => roles(&["todo:write"]),
        CrudAction::Read => roles(&["todo:read"]),
        CrudAction::Delete => roles(&["todo:admin"]),
    })
}

type Service = SaaSCqrsService<CallerIdentity, TodoCommand, Todo, String, TodoNotification>;
type TodoApp = SaaSCqrsApp<CallerIdentity, TodoCommand, Todo, String, TodoNotification, ()>;

// ---------------------------------------------------------------------------
// 3. SAAS SERVICE (write side: guards are automatic)
// ---------------------------------------------------------------------------

fn todo_service(service: &Service) -> TodoApp {
    let saas = service.saas().clone();
    saas.clone().guarded_router(move |cmd| {
        let s = saas.clone();
        match cmd {
            TodoCommand::Create(title) => {
                let s2 = s.clone();
                let logic = s.auth().and_then(move |c| {
                    let s3 = s2.clone();
                    let title = title.clone();
                    s2.aggregate_id().and_then(move |id| {
                        let tenant = c.tenant_id.value().to_string();
                        s3.set(CrudState::active(
                            c.tenant_id.clone(),
                            c.user_id.clone(),
                            Todo {
                                title: title.clone(),
                                completed: false,
                            },
                        ))
                        .then(s3.publish([TodoNotification::Created {
                            entity_id: id,
                            tenant_id: tenant,
                            title: title.clone(),
                        }]))
                    })
                });
                (CrudAction::Create, logic)
            }
            TodoCommand::Complete => {
                let s2 = s.clone();
                let logic = s.aggregate_id().and_then(move |id| {
                    let s3 = s2.clone();
                    s2.entity_state().and_then(move |state| match state {
                        CrudState::Active {
                            tenant_id,
                            owner_id,
                            data,
                        } => s3
                            .set(CrudState::active(
                                tenant_id.clone(),
                                owner_id,
                                Todo {
                                    completed: true,
                                    ..data
                                },
                            ))
                            .then(s3.publish([TodoNotification::Completed {
                                entity_id: id.clone(),
                                tenant_id: tenant_id.into_string(),
                            }])),
                        _ => s3.reject("Todo not found".to_string()),
                    })
                });
                (CrudAction::Update, logic)
            }
            TodoCommand::Delete => {
                let s2 = s.clone();
                let logic = s.aggregate_id().and_then(move |id| {
                    let s3 = s2.clone();
                    s2.entity_state().and_then(move |state| match state {
                        CrudState::Active {
                            tenant_id,
                            owner_id,
                            ..
                        } => s3
                            .set(CrudState::deleted(tenant_id.clone(), owner_id))
                            .then(s3.publish([TodoNotification::Deleted {
                                entity_id: id.clone(),
                                tenant_id: tenant_id.into_string(),
                            }])),
                        _ => s3.reject("Todo not found".to_string()),
                    })
                });
                (CrudAction::Delete, logic)
            }
        }
    })
}

// ---------------------------------------------------------------------------
// 4. ADMIN SERVICE (write side: guards bypassed for the super-admin)
// ---------------------------------------------------------------------------

fn todo_admin_service(service: &Service) -> TodoApp {
    let saas = service.saas().clone();
    saas.clone().unsafe_unguarded_router(move |cmd| {
        let s = saas.clone();
        match cmd {
            // The super-admin can delete any todo across any tenant.
            TodoCommand::Delete => s.entity_state().and_then(move |state| match state {
                CrudState::Active {
                    tenant_id,
                    owner_id,
                    ..
                } => s.set(CrudState::deleted(tenant_id, owner_id)),
                _ => s.reject("Todo not found".to_string()),
            }),
            _ => s.reject("Admin service only supports Delete".to_string()),
        }
    })
}

// ---------------------------------------------------------------------------
// 5. SQL READ PROJECTION (via the notification handler)
// ---------------------------------------------------------------------------

const READ_MODEL_DDL: &str = "CREATE TABLE IF NOT EXISTS todos_read (
  id         text        NOT NULL,
  tenant_id  text        NOT NULL,
  title      text        NOT NULL,
  completed  boolean     NOT NULL DEFAULT false,
  deleted    boolean     NOT NULL DEFAULT false,
  PRIMARY KEY (id)
)";

/// Called for each notification inside the save transaction; maintains a
/// denormalised read-model table optimised for queries.
fn projection() -> SqlxHandler<TodoNotification> {
    SqlxHandler::new(|ns, conn| {
        Box::pin(async move {
            for n in ns.iter() {
                let query = match n {
                    TodoNotification::Created {
                        entity_id,
                        tenant_id,
                        title,
                    } => sqlx::query(
                        "INSERT INTO todos_read (id, tenant_id, title, completed, deleted) VALUES ($1, $2, $3, false, false) ON CONFLICT (id) DO UPDATE SET title = EXCLUDED.title, completed = false, deleted = false",
                    )
                    .bind(entity_id)
                    .bind(tenant_id)
                    .bind(title),
                    TodoNotification::Completed { entity_id, .. } => {
                        sqlx::query("UPDATE todos_read SET completed = true WHERE id = $1").bind(entity_id)
                    }
                    TodoNotification::Deleted { entity_id, .. } => {
                        sqlx::query("UPDATE todos_read SET deleted = true WHERE id = $1").bind(entity_id)
                    }
                };
                query
                    .execute(&mut *conn)
                    .await
                    .map_err(BackendError::unknown)?;
            }
            Ok(())
        })
    })
}

// ---------------------------------------------------------------------------
// 6. SQL READ QUERIES (tenant-scoped by default)
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
struct TodoReadModel {
    id: String,
    tenant_id: String,
    title: String,
    completed: bool,
}

/// Lists the todos of the caller's tenant: the tenant filter is structural.
fn list_by_tenant(pool: PgPool) -> ScopedQueryFn<CallerIdentity, TodoReadModel, ()> {
    ScopedQueryFn::new(policy(), move |tenant, ()| {
        let pool = pool.clone();
        async move {
            sqlx::query_as::<_, TodoReadModel>(
                "SELECT id, tenant_id, title, completed FROM todos_read WHERE tenant_id = $1 AND deleted = false ORDER BY id",
            )
            .bind(tenant.value())
            .fetch_all(&pool)
            .await
            .unwrap_or_default()
        }
    })
}

/// Admin query: cross-tenant, no tenant filter.
fn admin_list_all(pool: PgPool) -> CrossTenantQueryFn<TodoReadModel, ()> {
    CrossTenantQueryFn::new(move |()| {
        let pool = pool.clone();
        async move {
            sqlx::query_as::<_, TodoReadModel>(
                "SELECT id, tenant_id, title, completed FROM todos_read WHERE deleted = false ORDER BY id",
            )
            .fetch_all(&pool)
            .await
            .unwrap_or_default()
        }
    })
}

// ---------------------------------------------------------------------------
// 7. WIRING
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = connect().await?;
    sqlx::query(READ_MODEL_DDL).execute(&pool).await?;

    let service = SaaSCqrsService::new(policy(), |m| m);
    let driver = SqlxCqrsDriver::for_namespace("todos", pool.clone()).await?;
    let backend = Backend::builder(TodoModel, service.domain())
        .driver(driver)
        .with_event_handler(projection())
        .build_default()
        .await?;
    let run = backend.compile(todo_service(&service));
    let admin = backend.compile(todo_admin_service(&service));

    let tenant_a = CallerIdentity::new("acme-corp", "alice", ["todo:write", "todo:read"]);
    let tenant_b = CallerIdentity::new("globex", "bob", ["todo:write", "todo:read"]);
    let super_admin = CallerIdentity::new("platform", "root", Vec::<String>::new());
    let todo = format!("todo-{}", uuid::Uuid::new_v4());
    let cmd = |caller: &CallerIdentity, c: TodoCommand| {
        command(&todo, SaaSCommand::new(caller.clone(), c))
    };

    // Tenant A creates a todo, then completes it.
    println!(
        "Create: {:?}",
        run(cmd(&tenant_a, TodoCommand::Create("Buy milk".into()))).await?
    );
    println!(
        "Complete: {:?}",
        run(cmd(&tenant_a, TodoCommand::Complete)).await?
    );
    // Tenant B tries to modify Tenant A's todo: REJECTED (tenant mismatch).
    println!(
        "Cross-tenant attempt: {:?}",
        run(cmd(&tenant_b, TodoCommand::Complete)).await?
    );
    // Tenant A lacks todo:admin: REJECTED (missing role).
    println!(
        "Delete without role: {:?}",
        run(cmd(&tenant_a, TodoCommand::Delete)).await?
    );

    // Tenant-scoped and cross-tenant reads on the projection.
    println!(
        "Tenant A sees: {:?}",
        list_by_tenant(pool.clone()).query(&tenant_a, ()).await
    );
    println!(
        "Tenant B sees: {:?}",
        list_by_tenant(pool.clone()).query(&tenant_b, ()).await
    );
    println!(
        "Admin sees {} todo(s)",
        admin_list_all(pool.clone()).query(()).await.len()
    );

    // The super-admin deletes through the unguarded service.
    println!(
        "Admin delete: {:?}",
        admin(cmd(&super_admin, TodoCommand::Delete)).await?
    );
    println!("Done!");
    Ok(())
}
