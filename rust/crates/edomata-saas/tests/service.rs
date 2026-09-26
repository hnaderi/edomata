//! Port of `SaaSServiceSuite.scala` (services, `CrudState`, `types.rs` and
//! re-export coverage).

use edomata_core::{CqrsModel, DomainModel, RequestContext};
use edomata_saas::{
    CallerIdentity, CommandMessage, CrudAction, CrudState, Decision, MessageMetadata, NonEmpty,
    RoleBasedPolicy, SaaSCommand, SaaSCqrsApp, SaaSCqrsService, SaaSEsApp, SaaSEventSourcedService,
    TenantId, UserId,
};
use futures::executor::block_on;

fn caller() -> CallerIdentity {
    CallerIdentity::new("tenant-a", "user-a", Vec::<String>::new())
}

fn cmd(payload: &str) -> CommandMessage<SaaSCommand<CallerIdentity, String>> {
    CommandMessage::new(
        "c1",
        chrono::Utc::now(),
        "e1",
        SaaSCommand::new(caller(), payload.to_string()),
    )
}

// ---------------------------------------------------------------------------
// CQRS service
// ---------------------------------------------------------------------------

struct TestCqrsModel;

impl CqrsModel for TestCqrsModel {
    type State = CrudState<String>;
    type Rejection = String;

    fn initial(&self) -> CrudState<String> {
        CrudState::NonExistent
    }
}

type CqrsService = SaaSCqrsService<CallerIdentity, String, String, String, String>;
type CqrsApp = SaaSCqrsApp<CallerIdentity, String, String, String, String, ()>;

fn cqrs_service() -> CqrsService {
    SaaSCqrsService::new(RoleBasedPolicy::none(), |m| m)
}

fn update(
    data: &'static str,
) -> impl Fn(CrudState<String>) -> Result<CrudState<String>, NonEmpty<String>> {
    move |s| match s {
        CrudState::Active {
            tenant_id,
            owner_id,
            ..
        } => Ok(CrudState::Active {
            tenant_id,
            owner_id,
            data: data.to_string(),
        }),
        _ => Err(NonEmpty::new("not active".to_string())),
    }
}

fn cqrs_app(service: &CqrsService) -> CqrsApp {
    let saas = service.saas().clone();
    saas.clone().guarded_router(move |c| match c.as_str() {
        "create" => (
            CrudAction::Create,
            saas.set(CrudState::active(
                "tenant-a",
                "user-a",
                "created".to_string(),
            )),
        ),
        "update" => (CrudAction::Update, saas.modify_s(update("updated")).void()),
        "decideS" => (CrudAction::Update, saas.decide_s(update("decided")).void()),
        "eval" => (CrudAction::Read, saas.eval(|| async {})),
        _ => (CrudAction::Read, saas.unit()),
    })
}

fn data(r: Result<(CrudState<String>, ()), NonEmpty<String>>) -> String {
    match r {
        Ok((CrudState::Active { data, .. }, ())) => data,
        other => panic!("Expected Ok with Active, got: {other:?}"),
    }
}

#[test]
fn cqrs_service_domain_is_accessible() {
    let service = cqrs_service();
    let _domain = service.domain();
    let _ =
        edomata_core::CqrsModel::dsl::<SaaSCommand<CallerIdentity, String>, String>(&TestCqrsModel);
}

#[test]
fn cqrs_service_set_changes_state() {
    let service = cqrs_service();
    let r = block_on(cqrs_app(&service).run(cmd("create"), CrudState::NonExistent));
    assert_eq!(data(r.result), "created");
}

#[test]
fn cqrs_service_modify_s_updates_state() {
    let service = cqrs_service();
    let active = CrudState::active("tenant-a", "user-a", "old".to_string());
    let r = block_on(cqrs_app(&service).run(cmd("update"), active));
    assert_eq!(data(r.result), "updated");
}

#[test]
fn cqrs_service_decide_s_transitions_state() {
    let service = cqrs_service();
    let active = CrudState::active("tenant-a", "user-a", "old".to_string());
    let r = block_on(cqrs_app(&service).run(cmd("decideS"), active));
    assert_eq!(data(r.result), "decided");
}

#[test]
fn cqrs_service_decide_s_rejects_on_invalid_state() {
    let service = cqrs_service();
    // The guard rejects NonExistent for Update before the logic runs, as in
    // Scala (the Scala test only asserts a rejection).
    let r = block_on(cqrs_app(&service).run(cmd("decideS"), CrudState::NonExistent));
    assert!(r.result.is_err(), "{:?}", r.result);
}

#[test]
fn cqrs_service_eval_runs_effect() {
    let service = cqrs_service();
    let active = CrudState::active("tenant-a", "user-a", "data".to_string());
    let r = block_on(cqrs_app(&service).run(cmd("eval"), active));
    assert!(r.result.is_ok());
}

// ---------------------------------------------------------------------------
// Event-sourced service
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
enum TestEvent {
    Created(String),
    Updated(String),
}

struct TestEsModel;

impl DomainModel for TestEsModel {
    type State = CrudState<String>;
    type Event = TestEvent;
    type Rejection = String;

    fn initial(&self) -> CrudState<String> {
        CrudState::NonExistent
    }

    fn transition(
        &self,
        e: &TestEvent,
        s: CrudState<String>,
    ) -> Result<CrudState<String>, NonEmpty<String>> {
        match (e, s) {
            (TestEvent::Created(v), _) => Ok(CrudState::active("t", "u", v.clone())),
            (
                TestEvent::Updated(v),
                CrudState::Active {
                    tenant_id,
                    owner_id,
                    ..
                },
            ) => Ok(CrudState::Active {
                tenant_id,
                owner_id,
                data: v.clone(),
            }),
            (TestEvent::Updated(_), _) => Err(NonEmpty::new("not active".to_string())),
        }
    }
}

type EsService = SaaSEventSourcedService<CallerIdentity, String, String, TestEvent, String, String>;
type EsApp = SaaSEsApp<CallerIdentity, String, String, TestEvent, String, String, ()>;

fn es_app(service: &EsService) -> EsApp {
    let saas = service.saas().clone();
    saas.clone().guarded_router(move |c| match c.as_str() {
        "create" => (
            CrudAction::Create,
            saas.decide(Decision::accept(TestEvent::Created("new".to_string()))),
        ),
        _ => (CrudAction::Read, saas.unit()),
    })
}

#[test]
fn es_service_domain_is_accessible() {
    let service: EsService = SaaSEventSourcedService::new(RoleBasedPolicy::none(), |m| m);
    let _domain = service.domain();
    let _ = TestEsModel.dsl::<SaaSCommand<CallerIdentity, String>, String>();
}

#[test]
fn es_service_guarded_router_creates_events() {
    let service: EsService = SaaSEventSourcedService::new(RoleBasedPolicy::none(), |m| m);
    let ctx = RequestContext::new(cmd("create"), CrudState::NonExistent);
    let r = block_on(es_app(&service).run(ctx));
    match r.result {
        Decision::Accepted { events, .. } => {
            assert_eq!(events.to_vec(), vec![TestEvent::Created("new".to_string())]);
        }
        other => panic!("Expected Accepted, got: {other:?}"),
    }
    // The transition is consistent with the model.
    let s = TestEsModel
        .transition(
            &TestEvent::Created("new".to_string()),
            CrudState::NonExistent,
        )
        .unwrap();
    assert_eq!(s.data().map(String::as_str), Some("new"));
    assert_eq!(
        TestEsModel.transition(&TestEvent::Updated("x".to_string()), CrudState::NonExistent),
        Err(NonEmpty::new("not active".to_string()))
    );
}

// ---------------------------------------------------------------------------
// CrudState
// ---------------------------------------------------------------------------

#[test]
fn crud_state_non_existent_is_the_initial_state() {
    assert_eq!(TestCqrsModel.initial(), CrudState::NonExistent);
    assert_eq!(CrudState::<String>::default(), CrudState::NonExistent);
}

#[test]
fn crud_state_active_carries_tenant_owner_data() {
    let state = CrudState::active("tenant-a", "user-a", "data");
    match &state {
        CrudState::Active {
            tenant_id,
            owner_id,
            data,
        } => {
            assert_eq!(tenant_id, &TenantId::new("tenant-a"));
            assert_eq!(owner_id, &UserId::new("user-a"));
            assert_eq!(*data, "data");
        }
        other => panic!("Expected Active, got: {other:?}"),
    }
    assert_eq!(state.tenant_id(), Some(&TenantId::new("tenant-a")));
    assert_eq!(state.owner_id(), Some(&UserId::new("user-a")));
    assert_eq!(state.data(), Some(&"data"));
    assert!(state.is_active());
    assert_eq!(state.clone().map(str::len).data(), Some(&4));
}

#[test]
fn crud_state_deleted_carries_tenant_owner() {
    let state: CrudState<String> = CrudState::deleted("tenant-a", "user-a");
    match &state {
        CrudState::Deleted {
            tenant_id,
            owner_id,
        } => {
            assert_eq!(tenant_id.value(), "tenant-a");
            assert_eq!(owner_id.value(), "user-a");
        }
        other => panic!("Expected Deleted, got: {other:?}"),
    }
    assert!(!state.is_active());
    assert_eq!(state.data(), None);
    assert_eq!(CrudState::<String>::NonExistent.tenant_id(), None);
}

// ---------------------------------------------------------------------------
// types.rs
// ---------------------------------------------------------------------------

#[test]
fn tenant_id_round_trips() {
    let tid = TenantId::new("abc");
    assert_eq!(tid.value(), "abc");
    assert_eq!(tid.to_string(), "abc");
    assert_eq!(TenantId::from("abc"), tid);
    assert_eq!(String::from(tid.clone()), "abc");
    assert_eq!(tid.into_string(), "abc");
}

#[test]
fn user_id_round_trips() {
    let uid = UserId::new("xyz");
    assert_eq!(uid.value(), "xyz");
    assert_eq!(UserId::from("xyz".to_string()), uid);
    assert_eq!(uid.as_ref(), "xyz");
}

#[test]
fn saas_command_wraps_caller_and_payload() {
    let c = CallerIdentity::new("tenant-a", "user-a", ["r"]);
    let cmd = SaaSCommand::new(c.clone(), "my-payload");
    assert_eq!(cmd.auth, c);
    assert_eq!(cmd.payload, "my-payload");
}

#[test]
fn crud_action_has_four_distinct_cases() {
    let all = CrudAction::ALL;
    assert_eq!(all.len(), 4);
    let distinct: std::collections::BTreeSet<_> = all.iter().collect();
    assert_eq!(distinct.len(), 4);
}

#[cfg(feature = "serde")]
#[test]
fn serde_wire_format_matches_scala_upickle_like_shape() {
    // TenantId / UserId are transparent strings.
    assert_eq!(serde_json::to_string(&TenantId::new("t")).unwrap(), "\"t\"");
    let s: CrudState<String> = CrudState::active("t", "u", "d".to_string());
    let json = serde_json::to_value(&s).unwrap();
    assert_eq!(json["Active"]["tenant_id"], "t");
    assert_eq!(json["Active"]["owner_id"], "u");
    assert_eq!(json["Active"]["data"], "d");
    let back: CrudState<String> = serde_json::from_value(json).unwrap();
    assert_eq!(back, s);
    assert_eq!(
        serde_json::to_string(&CrudState::<String>::NonExistent).unwrap(),
        "\"NonExistent\""
    );
}

// ---------------------------------------------------------------------------
// Re-exports
// ---------------------------------------------------------------------------

#[test]
fn re_exports_are_accessible() {
    let c: edomata_saas::CommandMessage<String> =
        CommandMessage::new("id", chrono::Utc::now(), "addr", "pay".to_string());
    assert_eq!(c.id, "id");
    let d: edomata_saas::Decision<String, i32, ()> = Decision::accept(1);
    assert!(d.is_accepted());
    let m: edomata_saas::MessageMetadata = MessageMetadata::empty();
    assert_eq!(m.correlation, None);
    let _naming: edomata_saas::PGNaming = edomata_saas::PGNaming::prefixed_str("x").unwrap();
}
