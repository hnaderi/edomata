//! Port of `SaaSDomainDSLSuite.scala`.

use std::collections::BTreeSet;

use edomata_core::{CommandMessage, Decision, RequestContext, ResponseD};
use edomata_saas::{
    CallerIdentity, CrudAction, CrudState, RoleBasedPolicy, SaaSCommand, SaaSDomainDsl, SaaSEsApp,
};
use futures::executor::block_on;

type Ev = String;
type Rej = String;
type Notif = String;
type Dsl = SaaSDomainDsl<CallerIdentity, String, String, Ev, Rej, Notif>;
type App<T> = SaaSEsApp<CallerIdentity, String, String, Ev, Rej, Notif, T>;

fn roles(xs: &[&str]) -> BTreeSet<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

fn dsl() -> Dsl {
    SaaSDomainDsl::new(
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
fn active() -> CrudState<String> {
    CrudState::active("tenant-a", "user-a", "hello".to_string())
}

fn ctx(
    caller: CallerIdentity,
    state: CrudState<String>,
    payload: &str,
) -> RequestContext<SaaSCommand<CallerIdentity, String>, CrudState<String>> {
    RequestContext::new(
        CommandMessage::new(
            "cmd-1",
            chrono::Utc::now(),
            "entity-1",
            SaaSCommand::new(caller, payload.to_string()),
        ),
        state,
    )
}

fn run<T: Send + 'static>(
    app: &App<T>,
    caller: CallerIdentity,
    state: CrudState<String>,
    payload: &str,
) -> ResponseD<Rej, Ev, Notif, T> {
    block_on(app.run(ctx(caller, state, payload)))
}

fn guarded_app() -> App<()> {
    let d = dsl();
    d.clone().guarded_router(move |cmd| match cmd.as_str() {
        "create" => (
            CrudAction::Create,
            d.decide(Decision::accept("created".to_string())),
        ),
        "update" => (
            CrudAction::Update,
            d.decide(Decision::accept("updated".to_string())),
        ),
        "delete" => (
            CrudAction::Delete,
            d.reject("delete not allowed".to_string()),
        ),
        _ => (CrudAction::Read, d.unit()),
    })
}

#[test]
fn guarded_router_correct_tenant_and_roles_executes_logic() {
    let r = run(&guarded_app(), writer(), active(), "update");
    assert_eq!(
        r.result.events().map(|e| e.to_vec()),
        Some(vec!["updated".to_string()])
    );
}

#[test]
fn guarded_router_create_bypasses_tenant_check_on_non_existent() {
    let r = run(&guarded_app(), writer(), CrudState::NonExistent, "create");
    assert!(r.result.is_accepted(), "{:?}", r.result);
}

#[test]
fn guarded_router_wrong_tenant_is_rejected() {
    let r = run(&guarded_app(), other_tenant(), active(), "update");
    assert_eq!(
        r.result.rejections().map(|e| e.to_vec()),
        Some(vec!["Tenant mismatch".to_string()])
    );
}

#[test]
fn guarded_router_missing_roles_is_rejected() {
    let r = run(&guarded_app(), read_only(), active(), "update");
    assert_eq!(
        r.result.rejections().map(|e| e.to_vec()),
        Some(vec!["Missing roles: write".to_string()])
    );
}

#[test]
fn guarded_router_non_existent_is_rejected_for_non_create() {
    let r = run(&guarded_app(), writer(), CrudState::NonExistent, "update");
    assert_eq!(
        r.result.rejections().map(|e| e.to_vec()),
        Some(vec!["Entity not found".to_string()])
    );
}

fn unsafe_app() -> App<()> {
    let d = dsl();
    d.clone()
        .unsafe_unguarded_router(move |cmd| match cmd.as_str() {
            "update" => d.decide(Decision::accept("admin-updated".to_string())),
            _ => d.unit(),
        })
}

#[test]
fn unsafe_unguarded_router_bypasses_tenant_and_roles() {
    for caller in [other_tenant(), read_only()] {
        let r = run(&unsafe_app(), caller, active(), "update");
        assert_eq!(
            r.result.events().map(|e| e.to_vec()),
            Some(vec!["admin-updated".to_string()])
        );
    }
}

#[test]
fn guarded_runs_guard_then_logic() {
    let d = dsl();
    let app = d.guarded(
        CrudAction::Update,
        d.decide(Decision::accept("updated".to_string())),
    );
    assert!(run(&app, writer(), active(), "test").result.is_accepted());
    assert!(
        run(&app, other_tenant(), active(), "test")
            .result
            .is_rejected()
    );
}

#[test]
fn unsafe_unguarded_executes_without_guard() {
    let d = dsl();
    let app = d.unsafe_unguarded(d.decide(Decision::accept("updated".to_string())));
    assert!(
        run(&app, other_tenant(), active(), "test")
            .result
            .is_accepted()
    );
}

#[test]
fn readers_extract_context() {
    let d = dsl();
    assert_eq!(
        run(&d.auth(), writer(), active(), "test")
            .result
            .to_option(),
        Some(writer())
    );
    assert_eq!(
        run(&d.command(), writer(), active(), "my-command")
            .result
            .to_option(),
        Some("my-command".to_string())
    );
    assert_eq!(
        run(&d.entity_state(), writer(), active(), "test")
            .result
            .to_option(),
        Some(active())
    );
    assert_eq!(
        run(&d.aggregate_id(), writer(), active(), "test")
            .result
            .to_option(),
        Some("entity-1".to_string())
    );
    assert_eq!(
        run(&d.pure(42), writer(), active(), "test")
            .result
            .to_option(),
        Some(42)
    );
}

#[test]
fn unit_reject_decide_publish_validate() {
    let d = dsl();
    assert_eq!(
        run(&d.unit(), writer(), active(), "test").result,
        Decision::unit()
    );
    let r = run(
        &d.reject::<()>("bad request".to_string()),
        writer(),
        active(),
        "test",
    );
    assert!(
        r.result
            .rejections()
            .unwrap()
            .contains(&"bad request".to_string())
    );
    let r = run(
        &d.decide(Decision::accept("evt".to_string())),
        writer(),
        active(),
        "test",
    );
    assert_eq!(
        r.result.events().map(|e| e.to_vec()),
        Some(vec!["evt".to_string()])
    );
    let r = run(
        &d.publish(["notif-1".to_string(), "notif-2".to_string()]),
        writer(),
        active(),
        "test",
    );
    assert_eq!(
        r.notifications,
        vec!["notif-1".to_string(), "notif-2".to_string()]
    );
    assert_eq!(
        run(&d.validate(Ok(42)), writer(), active(), "test")
            .result
            .to_option(),
        Some(42)
    );
    assert!(
        run(
            &d.validate::<i32>(Err(edomata_core::NonEmpty::new("err".to_string()))),
            writer(),
            active(),
            "test"
        )
        .result
        .is_rejected()
    );
    let evaluated = run(&d.eval(|| async { 7 }), writer(), active(), "test");
    assert_eq!(evaluated.result.to_option(), Some(7));
}

#[test]
fn guarded_router_no_notifications_on_guard_rejection() {
    let d = dsl();
    let app = d.clone().guarded_router(move |_| {
        (
            CrudAction::Update,
            d.publish(["should-not-appear".to_string()]),
        )
    });
    let r = run(&app, other_tenant(), active(), "test");
    assert!(r.result.is_rejected());
    assert!(r.notifications.is_empty());
}

#[test]
fn guarded_router_notifications_emitted_on_success() {
    let d = dsl();
    let app = d.clone().guarded_router(move |_| {
        let logic = d
            .decide(Decision::accept("evt".to_string()))
            .then(d.publish(["notif".to_string()]));
        (CrudAction::Update, logic)
    });
    let r = run(&app, writer(), active(), "test");
    assert!(r.result.is_accepted());
    assert_eq!(r.notifications, vec!["notif".to_string()]);
}

#[test]
fn guarded_router_deleted_state_tenant_rules() {
    let d = dsl();
    let deleted: CrudState<String> = CrudState::deleted("tenant-a", "user-a");
    let app = d
        .clone()
        .guarded_router(move |_| (CrudAction::Read, d.unit()));
    assert_eq!(
        run(&app, writer(), deleted.clone(), "test").result,
        Decision::unit()
    );
    assert!(
        run(&app, other_tenant(), deleted, "test")
            .result
            .is_rejected()
    );
}
