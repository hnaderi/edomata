//! Samples of the "SaaS multi-tenant module" chapter.

use std::collections::BTreeSet;

use edomata_core::CqrsModel;
use edomata_saas::*;
use serde::{Deserialize, Serialize};

// ANCHOR: types
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Todo {
    pub title: String,
    pub completed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TodoCommand {
    Create(String),
    Complete,
    Delete,
}

/// The CQRS model: the state is `CrudState<Todo>`.
pub struct TodoModel;

impl CqrsModel for TodoModel {
    type State = CrudState<Todo>;
    type Rejection = String;

    fn initial(&self) -> CrudState<Todo> {
        CrudState::NonExistent
    }
}
// ANCHOR_END: types

// ANCHOR: policy
fn roles(xs: &[&str]) -> BTreeSet<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

/// A role-based policy for `CallerIdentity`: which roles each action needs.
pub fn policy() -> RoleBasedPolicy {
    RoleBasedPolicy::new(|action| match action {
        CrudAction::Create => roles(&["todo:write"]),
        CrudAction::Read => roles(&["todo:read"]),
        CrudAction::Update => roles(&["todo:write"]),
        CrudAction::Delete => roles(&["todo:admin"]),
    })
}
// ANCHOR_END: policy

// ANCHOR: service
pub type TodoService = SaaSCqrsService<CallerIdentity, TodoCommand, Todo, String, String>;
pub type TodoApp = SaaSCqrsApp<CallerIdentity, TodoCommand, Todo, String, String, ()>;

/// The service: `guarded_router` runs the tenant and authorization checks
/// before each branch; guard failures become rejections through `mk_rejection`.
pub fn todo_service() -> TodoService {
    SaaSCqrsService::new(policy(), |message| message)
}

pub fn todo_app(service: &TodoService) -> TodoApp {
    let saas = service.saas().clone();
    saas.clone().guarded_router(move |command| {
        let s = saas.clone();
        match command {
            TodoCommand::Create(title) => {
                let s2 = s.clone();
                (
                    CrudAction::Create,
                    s.auth().and_then(move |caller| {
                        s2.set(CrudState::active(
                            caller.tenant_id,
                            caller.user_id,
                            Todo {
                                title: title.clone(),
                                completed: false,
                            },
                        ))
                    }),
                )
            }
            TodoCommand::Complete => (
                CrudAction::Update,
                s.modify_s(|state: CrudState<Todo>| match state {
                    CrudState::Active {
                        tenant_id,
                        owner_id,
                        data,
                    } => Ok(CrudState::active(
                        tenant_id,
                        owner_id,
                        Todo {
                            completed: true,
                            ..data
                        },
                    )),
                    _ => Err(NonEmpty::new("Not found".to_string())),
                })
                .void(),
            ),
            TodoCommand::Delete => {
                let s2 = s.clone();
                (
                    CrudAction::Delete,
                    s.entity_state().and_then(move |state| match state {
                        CrudState::Active {
                            tenant_id,
                            owner_id,
                            ..
                        } => s2.set(CrudState::deleted(tenant_id, owner_id)),
                        _ => s2.reject("Not found".to_string()),
                    }),
                )
            }
        }
    })
}
// ANCHOR_END: service

// ANCHOR: custom_auth
/// Any type can be the auth context: implement `AuthPolicy` for it.
#[derive(Clone, Debug)]
pub struct JwtClaims {
    pub sub: String,
    pub tenant_id: String,
    pub scopes: Vec<String>,
}

pub struct JwtPolicy;

impl AuthPolicy<JwtClaims> for JwtPolicy {
    fn tenant_id(&self, auth: &JwtClaims) -> TenantId {
        TenantId::new(&auth.tenant_id)
    }

    fn authorize(&self, auth: &JwtClaims, action: CrudAction) -> Result<(), String> {
        let required = match action {
            CrudAction::Create | CrudAction::Update => "write",
            CrudAction::Read => "read",
            CrudAction::Delete => "admin",
        };
        if auth.scopes.iter().any(|s| s == required) {
            Ok(())
        } else {
            Err(format!("Missing scope: {required}"))
        }
    }
}
// ANCHOR_END: custom_auth

// ANCHOR: admin
/// Administrative endpoints bypass the guards explicitly: the `unsafe_`
/// prefix makes every bypass grep-able.
pub fn admin_app(service: &TodoService) -> TodoApp {
    let saas = service.saas().clone();
    saas.clone()
        .unsafe_unguarded_router(move |command| match command {
            // No tenant / authorization check: the admin can delete anything.
            TodoCommand::Delete => saas.set(CrudState::NonExistent),
            // Re-enable the guard for the other commands.
            _ => saas.guarded(CrudAction::Update, saas.unit()),
        })
}
// ANCHOR_END: admin

// ANCHOR: reads
/// Tenant-scoped reads: the caller is a required parameter, so the tenant
/// filter cannot be forgotten.
pub fn list_todos(
    pool: edomata_sqlx::PgPool,
) -> ScopedQueryFn<CallerIdentity, (String, String), ()> {
    ScopedQueryFn::new(policy(), move |tenant, ()| {
        let pool = pool.clone();
        async move {
            sqlx::query_as::<_, (String, String)>(
                "SELECT id, title FROM todos_read WHERE tenant_id = $1 AND deleted = false",
            )
            .bind(tenant.value())
            .fetch_all(&pool)
            .await
            .unwrap_or_default()
        }
    })
}

/// Admin dashboards query across tenants explicitly.
pub fn admin_list_all(pool: edomata_sqlx::PgPool) -> CrossTenantQueryFn<(String, String), ()> {
    CrossTenantQueryFn::new(move |()| {
        let pool = pool.clone();
        async move {
            sqlx::query_as::<_, (String, String)>(
                "SELECT id, title FROM todos_read WHERE deleted = false",
            )
            .fetch_all(&pool)
            .await
            .unwrap_or_default()
        }
    })
}
// ANCHOR_END: reads

// ANCHOR: wiring
/// The tenant-aware driver fills the `tenant_id` / `owner_id` columns.
pub async fn wiring(pool: edomata_sqlx::PgPool) -> Result<(), edomata_backend::BackendError> {
    use edomata_backend::cqrs::Backend;
    use edomata_saas_sqlx::{SaaSCodec, SaaSSqlxCqrsDriver};

    let naming = PGNaming::prefixed_str("todos")
        .map_err(|e| edomata_backend::BackendError::persistence(e.to_string()))?;
    // DDL for migration tools, with optional Row-Level Security.
    for statement in SaaSPGSchema::cqrs_with(
        &naming,
        "jsonb",
        "jsonb",
        Some(&RlsConfig::new("app_user", "app.tenant_id")),
    ) {
        println!("{statement}");
    }

    let service = todo_service();
    let driver = SaaSSqlxCqrsDriver::new(naming, pool).await?;
    let backend = Backend::builder(TodoModel, service.domain())
        .driver(driver)
        .build(SaaSCodec::jsonb_state(), SaaSCodec::jsonb_notification())
        .await?;
    let _handle = backend.compile(todo_app(&service));
    Ok(())
}
// ANCHOR_END: wiring

#[cfg(test)]
mod tests {
    use super::*;
    use edomata_core::CommandMessage;

    fn cmd(
        caller: CallerIdentity,
        command: TodoCommand,
    ) -> CommandMessage<SaaSCommand<CallerIdentity, TodoCommand>> {
        CommandMessage::new(
            "cmd-1",
            chrono::Utc::now(),
            "todo-1",
            SaaSCommand::new(caller, command),
        )
    }

    #[test]
    fn guards_run_before_the_logic() {
        // ANCHOR: guard_tests
        let service = todo_service();
        let app = todo_app(&service);
        let alice = CallerIdentity::new("acme", "alice", ["todo:write", "todo:read"]);
        let bob = CallerIdentity::new("globex", "bob", ["todo:write", "todo:read"]);

        let created = futures::executor::block_on(app.run(
            cmd(alice.clone(), TodoCommand::Create("Buy milk".to_string())),
            CrudState::NonExistent,
        ));
        let (state, ()) = created.result.expect("alice may create");
        assert_eq!(state.tenant_id(), Some(&TenantId::new("acme")));

        // Bob belongs to another tenant: rejected before the logic runs.
        let cross =
            futures::executor::block_on(app.run(cmd(bob, TodoCommand::Complete), state.clone()));
        assert_eq!(
            cross.result.unwrap_err(),
            NonEmpty::new("Tenant mismatch".to_string())
        );

        // Alice lacks the `todo:admin` role.
        let forbidden =
            futures::executor::block_on(app.run(cmd(alice, TodoCommand::Delete), state));
        assert_eq!(
            forbidden.result.unwrap_err(),
            NonEmpty::new("Missing roles: todo:admin".to_string())
        );
        // ANCHOR_END: guard_tests
    }

    #[test]
    fn custom_policy() {
        let claims = JwtClaims {
            sub: "u".into(),
            tenant_id: "t".into(),
            scopes: vec!["read".into()],
        };
        assert_eq!(JwtPolicy.tenant_id(&claims), TenantId::new("t"));
        assert_eq!(JwtPolicy.authorize(&claims, CrudAction::Read), Ok(()));
        assert_eq!(
            JwtPolicy.authorize(&claims, CrudAction::Delete),
            Err("Missing scope: admin".to_string())
        );
    }

    #[test]
    fn admin_bypass() {
        let service = todo_service();
        let app = admin_app(&service);
        let stranger = CallerIdentity::new("other", "x", Vec::<String>::new());
        let state = CrudState::active(
            "acme",
            "alice",
            Todo {
                title: "t".into(),
                completed: false,
            },
        );
        let deleted =
            futures::executor::block_on(app.run(cmd(stranger, TodoCommand::Delete), state));
        assert_eq!(deleted.result.map(|(s, ())| s), Ok(CrudState::NonExistent));
    }
}
