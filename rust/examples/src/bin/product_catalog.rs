//! Port of `examples/src/main/scala/ProductCatalogExample.scala`: a
//! multi-tenant product catalogue with a custom authentication context (API
//! key scopes), typed rejections, a lifecycle state machine, an admin bypass
//! and the tenant-aware SaaS driver (`tenant_id` / `owner_id` columns).
//!
//! ```sh
//! cargo run -p edomata-examples --bin product_catalog
//! ```

use std::collections::BTreeSet;

use edomata_backend::BackendError;
use edomata_backend::cqrs::Backend;
use edomata_core::CqrsModel;
use edomata_examples::{command, connect};
use edomata_saas::*;
use edomata_saas_sqlx::{SaaSCodec, SaaSSqlxCqrsDriver, SqlxHandler, TenantStateLister};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// 1. DOMAIN TYPES
// ---------------------------------------------------------------------------

/// Custom authentication context based on API key scopes (not
/// `CallerIdentity`): any type works as long as an `AuthPolicy` exists.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ApiKeyContext {
    #[allow(dead_code)]
    api_key: String,
    tenant_id: TenantId,
    owner_id: UserId,
    scopes: BTreeSet<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Product {
    name: String,
    description: String,
    price_cents: i64,
    currency: String,
    status: ProductStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ProductCommand {
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all_fields = "camelCase")]
enum ProductNotification {
    Created { name: String, price_cents: i64 },
    DetailsUpdated { name: String },
    PriceUpdated { price_cents: i64, currency: String },
    Published {},
    Archived {},
    Deleted {},
}

/// Typed rejections: guard errors are mapped by `mk_rejection`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum ProductRejection {
    NotFound,
    InvalidTransition { from: String, to: String },
    InvalidPrice { reason: String },
    Unauthorized { reason: String },
}

// ---------------------------------------------------------------------------
// 2. CQRS MODEL + AUTH POLICY
// ---------------------------------------------------------------------------

struct ProductModel;

impl CqrsModel for ProductModel {
    type State = CrudState<Product>;
    type Rejection = ProductRejection;

    fn initial(&self) -> CrudState<Product> {
        CrudState::NonExistent
    }
}

/// Scope-based authorisation: `catalog:read`, `catalog:write`, `catalog:admin`.
struct ScopePolicy;

impl AuthPolicy<ApiKeyContext> for ScopePolicy {
    fn tenant_id(&self, auth: &ApiKeyContext) -> TenantId {
        auth.tenant_id.clone()
    }

    fn authorize(&self, auth: &ApiKeyContext, action: CrudAction) -> Result<(), String> {
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

/// Maps guard error strings to typed rejections.
fn mk_rejection(msg: String) -> ProductRejection {
    if msg.contains("Tenant mismatch") || msg.contains("Entity not found") {
        ProductRejection::NotFound
    } else {
        ProductRejection::Unauthorized { reason: msg }
    }
}

type Service =
    SaaSCqrsService<ApiKeyContext, ProductCommand, Product, ProductRejection, ProductNotification>;
type Dsl =
    SaaSCqrsDsl<ApiKeyContext, ProductCommand, Product, ProductRejection, ProductNotification>;
type ProductApp =
    SaaSCqrsApp<ApiKeyContext, ProductCommand, Product, ProductRejection, ProductNotification, ()>;

// ---------------------------------------------------------------------------
// 3. SAAS SERVICE (write side: guards are automatic)
// ---------------------------------------------------------------------------

fn price_check(d: &Dsl, price: i64) -> ProductApp {
    if price <= 0 {
        d.reject(ProductRejection::InvalidPrice {
            reason: "Price must be positive".into(),
        })
    } else {
        d.unit()
    }
}

/// Applies `f` to an active product, rejecting with `NotFound` otherwise.
fn on_active<F>(d: &Dsl, f: F) -> ProductApp
where
    F: Fn(&Dsl, TenantId, UserId, Product) -> ProductApp + Send + Sync + 'static,
{
    let d2 = d.clone();
    d.entity_state().and_then(move |state| match state {
        CrudState::Active {
            tenant_id,
            owner_id,
            data,
        } => f(&d2, tenant_id, owner_id, data),
        _ => d2.reject(ProductRejection::NotFound),
    })
}

fn set_active(
    d: &Dsl,
    tid: TenantId,
    oid: UserId,
    p: Product,
    n: ProductNotification,
) -> ProductApp {
    d.set(CrudState::active(tid, oid, p)).then(d.publish([n]))
}

fn product_service(service: &Service) -> ProductApp {
    let d = service.saas().clone();
    d.clone().guarded_router(move |cmd| match cmd {
        ProductCommand::Create {
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
                .then(d.publish([ProductNotification::Created {
                    name: n,
                    price_cents,
                }]));
            (CrudAction::Create, logic)
        }
        ProductCommand::UpdateDetails { name, description } => (
            CrudAction::Update,
            on_active(&d, move |d, tid, oid, p| {
                let p = Product {
                    name: name.clone(),
                    description: description.clone(),
                    ..p
                };
                set_active(
                    d,
                    tid,
                    oid,
                    p,
                    ProductNotification::DetailsUpdated { name: name.clone() },
                )
            }),
        ),
        ProductCommand::UpdatePrice {
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
                    ProductNotification::PriceUpdated {
                        price_cents,
                        currency: currency.clone(),
                    },
                )
            })),
        ),
        ProductCommand::Publish => (
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
                    ProductNotification::Published {},
                ),
                other => d.reject(ProductRejection::InvalidTransition {
                    from: other.name().into(),
                    to: "Published".into(),
                }),
            }),
        ),
        ProductCommand::Archive => (
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
                    ProductNotification::Archived {},
                ),
                other => d.reject(ProductRejection::InvalidTransition {
                    from: other.name().into(),
                    to: "Archived".into(),
                }),
            }),
        ),
        ProductCommand::Delete => (
            CrudAction::Delete,
            on_active(&d, |d, tid, oid, p| match p.status {
                ProductStatus::Published => d.reject(ProductRejection::InvalidTransition {
                    from: "Published".into(),
                    to: "Deleted".into(),
                }),
                _ => d
                    .set(CrudState::deleted(tid, oid))
                    .then(d.publish([ProductNotification::Deleted {}])),
            }),
        ),
    })
}

// ---------------------------------------------------------------------------
// 4. ADMIN SERVICE (write side: guards bypassed for the super-admin)
// ---------------------------------------------------------------------------

fn product_admin_service(service: &Service) -> ProductApp {
    let d = service.saas().clone();
    d.clone().unsafe_unguarded_router(move |cmd| match cmd {
        ProductCommand::Delete => on_active(&d, |d, tid, oid, _| {
            d.set(CrudState::deleted(tid, oid))
                .then(d.publish([ProductNotification::Deleted {}]))
        }),
        _ => d.reject(ProductRejection::Unauthorized {
            reason: "Admin service: Delete only".into(),
        }),
    })
}

// ---------------------------------------------------------------------------
// 5. SQL READ PROJECTION (via the notification handler)
//
// As in the Scala example, notifications do not carry the product id, so the
// projection uses placeholders; a real projection would put the id in the
// notifications (see the `saas_todo` example).
// ---------------------------------------------------------------------------

const READ_MODEL_DDL: &str = "CREATE TABLE IF NOT EXISTS products_read (
  id          text        NOT NULL PRIMARY KEY,
  tenant_id   text        NOT NULL,
  name        text        NOT NULL,
  description text        NOT NULL,
  price_cents bigint      NOT NULL,
  currency    text        NOT NULL,
  status      text        NOT NULL,
  deleted     boolean     NOT NULL DEFAULT false,
  created_at  timestamptz NOT NULL DEFAULT now(),
  updated_at  timestamptz NOT NULL DEFAULT now()
)";

fn projection() -> SqlxHandler<ProductNotification> {
    SqlxHandler::new(|ns, conn| {
        Box::pin(async move {
            for n in ns.iter() {
                let query = match n {
                    ProductNotification::Created { name, price_cents } => sqlx::query(
                        "INSERT INTO products_read (id, tenant_id, name, description, price_cents, currency, status) VALUES ($1, $2, $3, '', $4, 'USD', 'Draft') ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, price_cents = EXCLUDED.price_cents, status = 'Draft', deleted = false, updated_at = now()",
                    )
                    .bind("placeholder-id")
                    .bind("placeholder-tenant")
                    .bind(name)
                    .bind(price_cents),
                    ProductNotification::DetailsUpdated { name } => {
                        sqlx::query("UPDATE products_read SET name = $1, updated_at = now() WHERE id = $2")
                            .bind(name)
                            .bind("placeholder-id")
                    }
                    ProductNotification::PriceUpdated {
                        price_cents,
                        currency,
                    } => sqlx::query(
                        "UPDATE products_read SET price_cents = $1, currency = $2, updated_at = now() WHERE id = $3",
                    )
                    .bind(price_cents)
                    .bind(currency)
                    .bind("placeholder-id"),
                    ProductNotification::Published {} => {
                        sqlx::query("UPDATE products_read SET status = 'Published', updated_at = now() WHERE id = $1")
                            .bind("placeholder-id")
                    }
                    ProductNotification::Archived {} => {
                        sqlx::query("UPDATE products_read SET status = 'Archived', updated_at = now() WHERE id = $1")
                            .bind("placeholder-id")
                    }
                    ProductNotification::Deleted {} => {
                        sqlx::query("UPDATE products_read SET deleted = true, updated_at = now() WHERE id = $1")
                            .bind("placeholder-id")
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
// 6. WIRING
// ---------------------------------------------------------------------------

fn api_key(key: &str, tenant: &str, owner: &str, scopes: &[&str]) -> ApiKeyContext {
    ApiKeyContext {
        api_key: key.to_string(),
        tenant_id: TenantId::new(tenant),
        owner_id: UserId::new(owner),
        scopes: scopes.iter().map(|s| s.to_string()).collect(),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let naming = PGNaming::prefixed_str("catalog")?;
    // The DDL for migration tools (tenant-aware tables, optionally with RLS).
    println!("-- SaaSPGSchema::cqrs(catalog):");
    for statement in SaaSPGSchema::cqrs(&naming) {
        println!("{statement}");
    }

    let pool = connect().await?;
    sqlx::query(READ_MODEL_DDL).execute(&pool).await?;
    let service: Service = SaaSCqrsService::new(ScopePolicy, mk_rejection);
    let driver = SaaSSqlxCqrsDriver::new(naming, pool).await?;
    let backend = Backend::builder(ProductModel, service.domain())
        .driver(driver.clone())
        .with_event_handler(projection())
        .build(SaaSCodec::jsonb_state(), SaaSCodec::jsonb_notification())
        .await?;
    let run = backend.compile(product_service(&service));
    let _admin_run = backend.compile(product_admin_service(&service));

    let tenant_a = api_key(
        "key-acme-001",
        "acme-corp",
        "alice",
        &["catalog:read", "catalog:write"],
    );
    let tenant_b = api_key(
        "key-globex-001",
        "globex",
        "bob",
        &["catalog:read", "catalog:write"],
    );
    let admin = api_key(
        "key-admin-001",
        "acme-corp",
        "admin",
        &["catalog:read", "catalog:write", "catalog:admin"],
    );
    let product = format!("product-{}", uuid::Uuid::new_v4());
    let cmd = |caller: &ApiKeyContext, c: ProductCommand| {
        command(&product, SaaSCommand::new(caller.clone(), c))
    };

    // Tenant A creates a product (starts as Draft), updates the price, publishes.
    println!(
        "Create: {:?}",
        run(cmd(
            &tenant_a,
            ProductCommand::Create {
                name: "Widget".into(),
                description: "A fine widget".into(),
                price_cents: 1999,
                currency: "USD".into(),
            },
        ))
        .await?
    );
    println!(
        "Update price: {:?}",
        run(cmd(
            &tenant_a,
            ProductCommand::UpdatePrice {
                price_cents: 2499,
                currency: "USD".into(),
            },
        ))
        .await?
    );
    println!(
        "Publish: {:?}",
        run(cmd(&tenant_a, ProductCommand::Publish)).await?
    );
    // Tenant B tries to update Tenant A's product: REJECTED (tenant mismatch).
    println!(
        "Cross-tenant attempt: {:?}",
        run(cmd(
            &tenant_b,
            ProductCommand::UpdateDetails {
                name: "Hacked!".into(),
                description: "pwned".into(),
            },
        ))
        .await?
    );
    // Cannot delete a published product.
    println!(
        "Delete published: {:?}",
        run(cmd(&admin, ProductCommand::Delete)).await?
    );
    // Archive first, then delete.
    println!(
        "Archive: {:?}",
        run(cmd(&tenant_a, ProductCommand::Archive)).await?
    );
    println!(
        "Delete archived: {:?}",
        run(cmd(&admin, ProductCommand::Delete)).await?
    );

    // The tenant-aware driver fills tenant_id / owner_id, so states can be
    // listed per tenant.
    let lister = TenantStateLister::new(&driver, SaaSCodec::<CrudState<Product>>::jsonb_state());
    println!(
        "acme-corp has {} product state(s)",
        lister
            .list_by_tenant(&TenantId::new("acme-corp"))
            .await?
            .len()
    );
    println!("Done!");
    Ok(())
}
