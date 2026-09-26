//! Port of `ProductCatalogSuite.scala`: a realistic multi-tenant CQRS
//! service with a custom auth type, exercising the guarded and unguarded
//! routers end to end (without a backend).

use std::collections::BTreeSet;

use edomata_core::{CommandMessage, ResponseE};
use edomata_saas::{
    AuthPolicy, CrudAction, CrudState, SaaSCommand, SaaSCqrsApp, SaaSCqrsDsl, TenantId, UserId,
};
use futures::executor::block_on;

// --- Domain types -----------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProductStatus {
    Draft,
    Published,
    Archived,
}

impl ProductStatus {
    fn name(self) -> &'static str {
        match self {
            ProductStatus::Draft => "Draft",
            ProductStatus::Published => "Published",
            ProductStatus::Archived => "Archived",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Product {
    name: String,
    description: String,
    price_cents: i64,
    currency: String,
    status: ProductStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Cmd {
    Create {
        name: String,
        description: String,
        price_cents: i64,
        currency: String,
    },
    UpdateDetails {
        name: String,
        description: String,
    },
    UpdatePrice {
        price_cents: i64,
        currency: String,
    },
    Publish,
    Archive,
    Delete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Rejection {
    NotFound,
    #[allow(dead_code)]
    AlreadyExists,
    InvalidTransition(String, String),
    InvalidPrice(String),
    Unauthorized(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Notif {
    Created(String, i64),
    DetailsUpdated(String),
    PriceUpdated(i64, String),
    Published,
    Archived,
    Deleted,
}

// --- Custom auth (API key with scopes) ---------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
struct ApiKey {
    tenant_id: TenantId,
    owner_id: UserId,
    scopes: BTreeSet<String>,
}

struct ScopePolicy;

impl AuthPolicy<ApiKey> for ScopePolicy {
    fn tenant_id(&self, auth: &ApiKey) -> TenantId {
        auth.tenant_id.clone()
    }

    fn authorize(&self, auth: &ApiKey, action: CrudAction) -> Result<(), String> {
        let required = match action {
            CrudAction::Create | CrudAction::Update => "catalog:write",
            CrudAction::Read => "catalog:read",
            CrudAction::Delete => "catalog:admin",
        };
        if auth.scopes.contains(required) {
            Ok(())
        } else {
            Err(format!("Missing scope: {required}"))
        }
    }
}

// --- Fixtures ---------------------------------------------------------------

fn key(tenant: &str, scopes: &[&str]) -> ApiKey {
    ApiKey {
        tenant_id: TenantId::new(tenant),
        owner_id: UserId::new("user-a"),
        scopes: scopes.iter().map(|s| s.to_string()).collect(),
    }
}
fn writer() -> ApiKey {
    key("tenant-a", &["catalog:read", "catalog:write"])
}
fn reader() -> ApiKey {
    key("tenant-a", &["catalog:read"])
}
fn admin() -> ApiKey {
    key(
        "tenant-a",
        &["catalog:read", "catalog:write", "catalog:admin"],
    )
}
fn other_tenant() -> ApiKey {
    key(
        "tenant-b",
        &["catalog:read", "catalog:write", "catalog:admin"],
    )
}

fn mk_rejection(msg: String) -> Rejection {
    if msg.contains("Tenant mismatch") || msg.contains("Entity not found") {
        Rejection::NotFound
    } else {
        Rejection::Unauthorized(msg)
    }
}

type Dsl = SaaSCqrsDsl<ApiKey, Cmd, Product, Rejection, Notif>;
type App = SaaSCqrsApp<ApiKey, Cmd, Product, Rejection, Notif, ()>;
type State = CrudState<Product>;
type Out = ResponseE<Rejection, Notif, (State, ())>;

fn dsl() -> Dsl {
    SaaSCqrsDsl::new(ScopePolicy, mk_rejection)
}

fn product(status: ProductStatus) -> Product {
    Product {
        name: "Widget".into(),
        description: "A fine widget".into(),
        price_cents: 1999,
        currency: "USD".into(),
        status,
    }
}
fn active(status: ProductStatus) -> State {
    CrudState::active("tenant-a", "user-a", product(status))
}
fn deleted() -> State {
    CrudState::deleted("tenant-a", "user-a")
}

fn run(app: &App, caller: ApiKey, state: State, payload: Cmd) -> Out {
    let cmd = CommandMessage::new(
        "cmd-1",
        chrono::Utc::now(),
        "product-1",
        SaaSCommand::new(caller, payload),
    );
    block_on(app.run(cmd, state))
}

fn product_of(out: &Out) -> &Product {
    match &out.result {
        Ok((CrudState::Active { data, .. }, ())) => data,
        other => panic!("Unexpected: {other:?}"),
    }
}

fn rejections(out: &Out) -> Vec<Rejection> {
    match &out.result {
        Err(errs) => errs.to_vec(),
        other => panic!("Expected rejection, got: {other:?}"),
    }
}

fn is_deleted(out: &Out) -> bool {
    matches!(&out.result, Ok((CrudState::Deleted { .. }, ())))
}

fn price_check(d: &Dsl, price: i64) -> App {
    if price <= 0 {
        d.reject(Rejection::InvalidPrice("Price must be positive".into()))
    } else {
        d.unit()
    }
}

/// Applies `f` to an active product, rejecting with `NotFound` otherwise.
fn on_active<F>(d: &Dsl, f: F) -> App
where
    F: Fn(&Dsl, TenantId, UserId, Product) -> App + Send + Sync + 'static,
{
    let d2 = d.clone();
    d.entity_state().and_then(move |state| match state {
        CrudState::Active {
            tenant_id,
            owner_id,
            data,
        } => f(&d2, tenant_id, owner_id, data),
        _ => d2.reject(Rejection::NotFound),
    })
}

fn set_active(d: &Dsl, tid: TenantId, oid: UserId, p: Product, n: Notif) -> App {
    d.set(CrudState::active(tid, oid, p)).then(d.publish([n]))
}

// --- Guarded router (the main service logic) --------------------------------

fn service_app() -> App {
    let d = dsl();
    d.clone().guarded_router(move |cmd| match cmd {
        Cmd::Create {
            name,
            description,
            price_cents,
            currency,
        } => {
            let d2 = d.clone();
            let n = name.clone();
            let logic = price_check(&d, price_cents)
                .then(d.auth().and_then(move |c| {
                    d2.set(CrudState::active(
                        c.tenant_id,
                        c.owner_id,
                        Product {
                            name: name.clone(),
                            description: description.clone(),
                            price_cents,
                            currency: currency.clone(),
                            status: ProductStatus::Draft,
                        },
                    ))
                }))
                .then(d.publish([Notif::Created(n, price_cents)]));
            (CrudAction::Create, logic)
        }
        Cmd::UpdateDetails { name, description } => (
            CrudAction::Update,
            on_active(&d, move |d, tid, oid, p| {
                let p = Product {
                    name: name.clone(),
                    description: description.clone(),
                    ..p
                };
                set_active(d, tid, oid, p, Notif::DetailsUpdated(name.clone()))
            }),
        ),
        Cmd::UpdatePrice {
            price_cents,
            currency,
        } => (
            CrudAction::Update,
            price_check(&d, price_cents).then(on_active(&d, move |d, tid, oid, p| {
                let p = Product {
                    price_cents,
                    currency: currency.clone(),
                    ..p
                };
                set_active(
                    d,
                    tid,
                    oid,
                    p,
                    Notif::PriceUpdated(price_cents, currency.clone()),
                )
            })),
        ),
        Cmd::Publish => (
            CrudAction::Update,
            on_active(&d, |d, tid, oid, p| match p.status {
                ProductStatus::Draft | ProductStatus::Archived => set_active(
                    d,
                    tid,
                    oid,
                    Product {
                        status: ProductStatus::Published,
                        ..p
                    },
                    Notif::Published,
                ),
                other => d.reject(Rejection::InvalidTransition(
                    other.name().into(),
                    "Published".into(),
                )),
            }),
        ),
        Cmd::Archive => (
            CrudAction::Update,
            on_active(&d, |d, tid, oid, p| match p.status {
                ProductStatus::Published => set_active(
                    d,
                    tid,
                    oid,
                    Product {
                        status: ProductStatus::Archived,
                        ..p
                    },
                    Notif::Archived,
                ),
                other => d.reject(Rejection::InvalidTransition(
                    other.name().into(),
                    "Archived".into(),
                )),
            }),
        ),
        Cmd::Delete => (
            CrudAction::Delete,
            on_active(&d, |d, tid, oid, p| match p.status {
                ProductStatus::Published => d.reject(Rejection::InvalidTransition(
                    "Published".into(),
                    "Deleted".into(),
                )),
                _ => d
                    .set(CrudState::deleted(tid, oid))
                    .then(d.publish([Notif::Deleted])),
            }),
        ),
    })
}

// --- Unguarded router (admin bypass) ----------------------------------------

fn admin_app() -> App {
    let d = dsl();
    d.clone().unsafe_unguarded_router(move |cmd| match cmd {
        Cmd::Delete => on_active(&d, |d, tid, oid, _| {
            d.set(CrudState::deleted(tid, oid))
                .then(d.publish([Notif::Deleted]))
        }),
        _ => d.reject(Rejection::Unauthorized("Admin: Delete only".into())),
    })
}

fn create(name: &str, price: i64) -> Cmd {
    Cmd::Create {
        name: name.into(),
        description: "desc".into(),
        price_cents: price,
        currency: "USD".into(),
    }
}

fn update_details(name: &str, description: &str) -> Cmd {
    Cmd::UpdateDetails {
        name: name.into(),
        description: description.into(),
    }
}

// =========================================================================
// 1. CRUD LIFECYCLE
// =========================================================================

#[test]
fn create_produces_draft_state_and_created_notification() {
    let out = run(
        &service_app(),
        writer(),
        CrudState::NonExistent,
        create("Widget", 1999),
    );
    let p = product_of(&out);
    assert_eq!(p.status, ProductStatus::Draft);
    assert_eq!(p.name, "Widget");
    assert_eq!(p.price_cents, 1999);
    assert_eq!(
        out.notifications,
        vec![Notif::Created("Widget".into(), 1999)]
    );
}

#[test]
fn update_details_changes_name_and_description() {
    let out = run(
        &service_app(),
        writer(),
        active(ProductStatus::Draft),
        update_details("New Name", "New Desc"),
    );
    let p = product_of(&out);
    assert_eq!(p.name, "New Name");
    assert_eq!(p.description, "New Desc");
    assert_eq!(
        out.notifications,
        vec![Notif::DetailsUpdated("New Name".into())]
    );
}

#[test]
fn update_price_changes_price_and_currency() {
    let out = run(
        &service_app(),
        writer(),
        active(ProductStatus::Draft),
        Cmd::UpdatePrice {
            price_cents: 2499,
            currency: "EUR".into(),
        },
    );
    let p = product_of(&out);
    assert_eq!(p.price_cents, 2499);
    assert_eq!(p.currency, "EUR");
    assert_eq!(
        out.notifications,
        vec![Notif::PriceUpdated(2499, "EUR".into())]
    );
}

#[test]
fn publish_draft_to_published() {
    let out = run(
        &service_app(),
        writer(),
        active(ProductStatus::Draft),
        Cmd::Publish,
    );
    assert_eq!(product_of(&out).status, ProductStatus::Published);
    assert_eq!(out.notifications, vec![Notif::Published]);
}

#[test]
fn archive_published_to_archived() {
    let out = run(
        &service_app(),
        writer(),
        active(ProductStatus::Published),
        Cmd::Archive,
    );
    assert_eq!(product_of(&out).status, ProductStatus::Archived);
    assert_eq!(out.notifications, vec![Notif::Archived]);
}

#[test]
fn republish_archived_to_published() {
    let out = run(
        &service_app(),
        writer(),
        active(ProductStatus::Archived),
        Cmd::Publish,
    );
    assert_eq!(product_of(&out).status, ProductStatus::Published);
}

#[test]
fn delete_draft_to_deleted() {
    let out = run(
        &service_app(),
        admin(),
        active(ProductStatus::Draft),
        Cmd::Delete,
    );
    assert!(is_deleted(&out), "{:?}", out.result);
    assert_eq!(out.notifications, vec![Notif::Deleted]);
}

#[test]
fn delete_archived_to_deleted() {
    let out = run(
        &service_app(),
        admin(),
        active(ProductStatus::Archived),
        Cmd::Delete,
    );
    assert!(is_deleted(&out), "{:?}", out.result);
}

// =========================================================================
// 2. STATE MACHINE VALIDATION
// =========================================================================

#[test]
fn cannot_publish_already_published_product() {
    let out = run(
        &service_app(),
        writer(),
        active(ProductStatus::Published),
        Cmd::Publish,
    );
    assert!(rejections(&out).contains(&Rejection::InvalidTransition(
        "Published".into(),
        "Published".into()
    )));
}

#[test]
fn cannot_archive_a_draft_product() {
    let out = run(
        &service_app(),
        writer(),
        active(ProductStatus::Draft),
        Cmd::Archive,
    );
    assert!(rejections(&out).contains(&Rejection::InvalidTransition(
        "Draft".into(),
        "Archived".into()
    )));
}

#[test]
fn cannot_delete_a_published_product() {
    let out = run(
        &service_app(),
        admin(),
        active(ProductStatus::Published),
        Cmd::Delete,
    );
    assert!(rejections(&out).contains(&Rejection::InvalidTransition(
        "Published".into(),
        "Deleted".into()
    )));
}

#[test]
fn cannot_update_a_deleted_product() {
    let out = run(
        &service_app(),
        writer(),
        deleted(),
        update_details("x", "y"),
    );
    assert!(out.result.is_err());
}

// =========================================================================
// 3. CUSTOM AuthPolicy ENFORCEMENT
// =========================================================================

#[test]
fn write_scope_can_create() {
    let out = run(
        &service_app(),
        writer(),
        CrudState::NonExistent,
        create("X", 100),
    );
    assert!(out.result.is_ok());
}

#[test]
fn read_only_scope_cannot_create() {
    let out = run(
        &service_app(),
        reader(),
        CrudState::NonExistent,
        create("X", 100),
    );
    let errs = rejections(&out);
    assert!(
        errs.iter()
            .any(|r| matches!(r, Rejection::Unauthorized(m) if m.contains("catalog:write"))),
        "{errs:?}"
    );
}

#[test]
fn read_only_scope_cannot_update() {
    let out = run(
        &service_app(),
        reader(),
        active(ProductStatus::Draft),
        update_details("x", "y"),
    );
    assert!(
        rejections(&out)
            .iter()
            .any(|r| matches!(r, Rejection::Unauthorized(_)))
    );
}

#[test]
fn write_scope_cannot_delete() {
    let out = run(
        &service_app(),
        writer(),
        active(ProductStatus::Draft),
        Cmd::Delete,
    );
    let errs = rejections(&out);
    assert!(
        errs.iter()
            .any(|r| matches!(r, Rejection::Unauthorized(m) if m.contains("catalog:admin"))),
        "{errs:?}"
    );
}

#[test]
fn admin_scope_can_delete() {
    let out = run(
        &service_app(),
        admin(),
        active(ProductStatus::Draft),
        Cmd::Delete,
    );
    assert!(out.result.is_ok());
}

// =========================================================================
// 4. MULTI-TENANT ISOLATION
// =========================================================================

#[test]
fn other_tenant_cannot_update_product() {
    let out = run(
        &service_app(),
        other_tenant(),
        active(ProductStatus::Draft),
        update_details("Hacked!", "pwned"),
    );
    assert!(rejections(&out).contains(&Rejection::NotFound));
}

#[test]
fn other_tenant_cannot_delete_product() {
    let out = run(
        &service_app(),
        other_tenant(),
        active(ProductStatus::Draft),
        Cmd::Delete,
    );
    assert!(rejections(&out).contains(&Rejection::NotFound));
}

// =========================================================================
// 5. ADMIN BYPASS (unsafe_unguarded_router)
// =========================================================================

#[test]
fn admin_bypass_other_tenant_can_delete_via_unguarded_router() {
    let out = run(
        &admin_app(),
        other_tenant(),
        active(ProductStatus::Draft),
        Cmd::Delete,
    );
    assert!(
        is_deleted(&out),
        "Expected Ok (bypass), got: {:?}",
        out.result
    );
}

#[test]
fn admin_bypass_read_only_scope_can_delete_via_unguarded_router() {
    let out = run(
        &admin_app(),
        reader(),
        active(ProductStatus::Draft),
        Cmd::Delete,
    );
    assert!(out.result.is_ok());
}

#[test]
fn admin_bypass_rejects_other_commands() {
    let out = run(
        &admin_app(),
        admin(),
        active(ProductStatus::Draft),
        Cmd::Publish,
    );
    assert_eq!(
        rejections(&out),
        vec![Rejection::Unauthorized("Admin: Delete only".into())]
    );
}

// =========================================================================
// 6. NOTIFICATIONS
// =========================================================================

#[test]
fn no_notifications_on_guard_rejection() {
    let out = run(
        &service_app(),
        other_tenant(),
        active(ProductStatus::Draft),
        update_details("x", "y"),
    );
    assert!(out.result.is_err());
    assert!(out.notifications.is_empty());
}

#[test]
fn no_notifications_on_business_logic_rejection() {
    let out = run(
        &service_app(),
        writer(),
        active(ProductStatus::Published),
        Cmd::Publish,
    );
    assert!(out.result.is_err());
    assert!(out.notifications.is_empty());
}

// =========================================================================
// 7. PRICE VALIDATION
// =========================================================================

#[test]
fn create_with_zero_price_is_rejected() {
    let out = run(
        &service_app(),
        writer(),
        CrudState::NonExistent,
        create("X", 0),
    );
    assert!(
        rejections(&out)
            .iter()
            .any(|r| matches!(r, Rejection::InvalidPrice(_)))
    );
}

#[test]
fn create_with_negative_price_is_rejected() {
    let out = run(
        &service_app(),
        writer(),
        CrudState::NonExistent,
        create("X", -100),
    );
    assert!(
        rejections(&out)
            .iter()
            .any(|r| matches!(r, Rejection::InvalidPrice(_)))
    );
}

#[test]
fn update_price_to_zero_is_rejected() {
    let out = run(
        &service_app(),
        writer(),
        active(ProductStatus::Draft),
        Cmd::UpdatePrice {
            price_cents: 0,
            currency: "USD".into(),
        },
    );
    assert!(
        rejections(&out)
            .iter()
            .any(|r| matches!(r, Rejection::InvalidPrice(_)))
    );
}
