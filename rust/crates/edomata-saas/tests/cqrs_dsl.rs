//! Port of `SaaSCQRSDSLSuite.scala`.

use std::collections::BTreeSet;

use edomata_core::{CommandMessage, ResponseE};
use edomata_saas::{
    CallerIdentity, CrudAction, CrudState, RoleBasedPolicy, SaaSCommand, SaaSCqrsApp, SaaSCqrsDsl,
};
use futures::executor::block_on;

type State = CrudState<String>;
type Dsl = SaaSCqrsDsl<CallerIdentity, String, String, String, String>;
type App = SaaSCqrsApp<CallerIdentity, String, String, String, String, ()>;

fn roles(xs: &[&str]) -> BTreeSet<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

fn dsl() -> Dsl {
    SaaSCqrsDsl::new(
        RoleBasedPolicy::new(|action| match action {
            CrudAction::Create | CrudAction::Update => roles(&["write"]),
            CrudAction::Read => roles(&["read"]),
            CrudAction::Delete => roles(&["admin"]),
        }),
        |msg| msg,
    )
}

fn writer() -> CallerIdentity {
    CallerIdentity::new("tenant-a", "user-a", ["read", "write"])
}
fn read_only() -> CallerIdentity {
    CallerIdentity::new("tenant-a", "user-a", ["read"])
}
fn other_tenant() -> CallerIdentity {
    CallerIdentity::new("tenant-b", "user-a", ["read", "write"])
}
fn active() -> State {
    CrudState::active("tenant-a", "user-a", "hello".to_string())
}

fn cmd(
    caller: CallerIdentity,
    payload: &str,
) -> CommandMessage<SaaSCommand<CallerIdentity, String>> {
    CommandMessage::new(
        "cmd-1",
        chrono::Utc::now(),
        "entity-1",
        SaaSCommand::new(caller, payload.to_string()),
    )
}

fn run(
    app: &App,
    caller: CallerIdentity,
    state: State,
    payload: &str,
) -> ResponseE<String, String, (State, ())> {
    block_on(app.run(cmd(caller, payload), state))
}

fn data_of(r: &ResponseE<String, String, (State, ())>) -> &str {
    match &r.result {
        Ok((CrudState::Active { data, .. }, ())) => data,
        other => panic!("Unexpected result: {other:?}"),
    }
}

fn guarded_app() -> App {
    let d = dsl();
    d.clone().guarded_router(move |c| match c.as_str() {
        "create" => (
            CrudAction::Create,
            d.set(CrudState::active(
                "tenant-a",
                "user-a",
                "created".to_string(),
            )),
        ),
        "update" => (
            CrudAction::Update,
            d.set(CrudState::active(
                "tenant-a",
                "user-a",
                "updated".to_string(),
            )),
        ),
        "delete" => (
            CrudAction::Delete,
            d.reject("delete not implemented".to_string()),
        ),
        _ => (CrudAction::Read, d.unit()),
    })
}

#[test]
fn guarded_router_correct_tenant_and_roles_executes_logic() {
    let r = run(&guarded_app(), writer(), active(), "update");
    assert_eq!(data_of(&r), "updated");
}

#[test]
fn guarded_router_create_bypasses_tenant_check_on_non_existent() {
    let r = run(&guarded_app(), writer(), CrudState::NonExistent, "create");
    assert_eq!(data_of(&r), "created");
}

#[test]
fn guarded_router_wrong_tenant_is_rejected() {
    let r = run(&guarded_app(), other_tenant(), active(), "update");
    let errs = r.result.unwrap_err();
    assert!(
        errs.iter().any(|e| e.contains("Tenant mismatch")),
        "{errs:?}"
    );
}

#[test]
fn guarded_router_missing_roles_is_rejected() {
    let r = run(&guarded_app(), read_only(), active(), "update");
    let errs = r.result.unwrap_err();
    assert!(errs.iter().any(|e| e.contains("Missing roles")), "{errs:?}");
}

#[test]
fn guarded_router_non_existent_is_rejected_for_non_create() {
    let r = run(&guarded_app(), writer(), CrudState::NonExistent, "update");
    let errs = r.result.unwrap_err();
    assert!(
        errs.iter().any(|e| e.contains("Entity not found")),
        "{errs:?}"
    );
}

fn unsafe_app() -> App {
    let d = dsl();
    d.clone()
        .unsafe_unguarded_router(move |c| match c.as_str() {
            "update" => d.set(CrudState::active(
                "tenant-a",
                "user-a",
                "admin-updated".to_string(),
            )),
            _ => d.unit(),
        })
}

#[test]
fn unsafe_unguarded_router_wrong_tenant_is_not_rejected() {
    let r = run(&unsafe_app(), other_tenant(), active(), "update");
    assert_eq!(data_of(&r), "admin-updated");
}

#[test]
fn unsafe_unguarded_router_missing_roles_is_not_rejected() {
    let r = run(&unsafe_app(), read_only(), active(), "update");
    assert!(r.result.is_ok(), "{:?}", r.result);
}

fn notify_app() -> App {
    let d = dsl();
    d.clone().guarded_router(move |c| match c.as_str() {
        "update" => {
            let logic = d
                .set(CrudState::active(
                    "tenant-a",
                    "user-a",
                    "updated".to_string(),
                ))
                .then(d.publish(["entity-updated".to_string()]));
            (CrudAction::Update, logic)
        }
        _ => (CrudAction::Read, d.unit()),
    })
}

#[test]
fn guarded_router_notifications_are_emitted_on_success() {
    let r = run(&notify_app(), writer(), active(), "update");
    assert!(r.result.is_ok());
    assert_eq!(r.notifications, vec!["entity-updated".to_string()]);
}

#[test]
fn guarded_router_no_notifications_on_guard_rejection() {
    let r = run(&notify_app(), other_tenant(), active(), "update");
    assert!(r.result.is_err());
    assert!(r.notifications.is_empty());
}

#[test]
fn readers_guard_and_helpers() {
    let d = dsl();
    assert_eq!(
        run(&d.auth().void(), writer(), active(), "x")
            .result
            .map(|(s, ())| s),
        Ok(active())
    );
    let r = block_on(d.command().run(cmd(writer(), "my-command"), active()));
    assert_eq!(r.result, Ok((active(), "my-command".to_string())));
    let r = block_on(d.entity_state().run(cmd(writer(), "x"), active()));
    assert_eq!(r.result, Ok((active(), active())));
    let r = block_on(d.aggregate_id().run(cmd(writer(), "x"), active()));
    assert_eq!(r.result, Ok((active(), "entity-1".to_string())));
    let r = block_on(d.pure(3).run(cmd(writer(), "x"), active()));
    assert_eq!(r.result, Ok((active(), 3)));
    let r = block_on(d.eval(|| async { 9 }).run(cmd(writer(), "x"), active()));
    assert_eq!(r.result, Ok((active(), 9)));
    let r = block_on(
        d.validate::<i32>(Err(edomata_saas::NonEmpty::new("bad".to_string())))
            .run(cmd(writer(), "x"), active()),
    );
    assert_eq!(
        r.result,
        Err(edomata_saas::NonEmpty::new("bad".to_string()))
    );
    let r = block_on(
        d.guard(CrudAction::Delete)
            .run(cmd(writer(), "x"), active()),
    );
    assert_eq!(
        r.result,
        Err(edomata_saas::NonEmpty::new(
            "Missing roles: admin".to_string()
        ))
    );
    let r = block_on(
        d.guarded(CrudAction::Read, d.unit())
            .run(cmd(writer(), "x"), active()),
    );
    assert_eq!(r.result, Ok((active(), ())));
    let r = block_on(
        d.unsafe_unguarded(d.unit())
            .run(cmd(other_tenant(), "x"), active()),
    );
    assert_eq!(r.result, Ok((active(), ())));
}

#[test]
fn modify_s_and_decide_s_transition_state() {
    let d = dsl();
    let upd = |s: State| match s {
        CrudState::Active {
            tenant_id,
            owner_id,
            ..
        } => Ok(CrudState::Active {
            tenant_id,
            owner_id,
            data: "modified".to_string(),
        }),
        _ => Err(edomata_saas::NonEmpty::new("not active".to_string())),
    };
    let r = block_on(d.modify_s(upd).run(cmd(writer(), "x"), active()));
    assert_eq!(
        r.result.as_ref().map(|(s, _)| s.data().cloned()),
        Ok(Some("modified".to_string()))
    );
    let r = block_on(
        d.decide_s(upd)
            .run(cmd(writer(), "x"), CrudState::NonExistent),
    );
    assert_eq!(
        r.result,
        Err(edomata_saas::NonEmpty::new("not active".to_string()))
    );
}
