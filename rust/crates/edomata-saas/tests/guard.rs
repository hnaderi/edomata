//! Port of `SaaSGuardSuite.scala`.

use std::collections::BTreeSet;

use edomata_saas::{
    AuthPolicy, CallerIdentity, CrudAction, CrudState, RoleBasedPolicy, SaaSGuard, TenantId,
};

fn roles(xs: &[&str]) -> BTreeSet<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

fn policy() -> RoleBasedPolicy {
    RoleBasedPolicy::new(|action| match action {
        CrudAction::Create => roles(&["write"]),
        CrudAction::Read => roles(&["read"]),
        CrudAction::Update => roles(&["write"]),
        CrudAction::Delete => roles(&["admin"]),
    })
}

fn caller_a() -> CallerIdentity {
    CallerIdentity::new("tenant-a", "user-a", ["read", "write"])
}
fn caller_b() -> CallerIdentity {
    CallerIdentity::new("tenant-b", "user-a", ["read", "write"])
}
fn caller_admin() -> CallerIdentity {
    CallerIdentity::new("tenant-a", "user-a", ["read", "write", "admin"])
}
fn caller_read_only() -> CallerIdentity {
    CallerIdentity::new("tenant-a", "user-a", ["read"])
}
fn active() -> CrudState<&'static str> {
    CrudState::active("tenant-a", "user-a", "some data")
}
fn deleted() -> CrudState<&'static str> {
    CrudState::deleted("tenant-a", "user-a")
}

#[test]
fn check_tenant_create_always_passes() {
    let p = policy();
    assert_eq!(
        SaaSGuard::check_tenant(
            &CrudState::<&str>::NonExistent,
            &caller_a(),
            CrudAction::Create,
            &p
        ),
        Ok(())
    );
    assert_eq!(
        SaaSGuard::check_tenant(&active(), &caller_a(), CrudAction::Create, &p),
        Ok(())
    );
}

#[test]
fn check_tenant_same_tenant_passes_for_update_delete_read() {
    let p = policy();
    for action in [CrudAction::Update, CrudAction::Delete, CrudAction::Read] {
        assert_eq!(
            SaaSGuard::check_tenant(&active(), &caller_a(), action, &p),
            Ok(())
        );
    }
}

#[test]
fn check_tenant_different_tenant_is_rejected() {
    let p = policy();
    for action in [CrudAction::Update, CrudAction::Delete, CrudAction::Read] {
        assert_eq!(
            SaaSGuard::check_tenant(&active(), &caller_b(), action, &p),
            Err("Tenant mismatch".to_string())
        );
    }
}

#[test]
fn check_tenant_non_existent_is_rejected_for_non_create() {
    let p = policy();
    for action in [CrudAction::Update, CrudAction::Delete, CrudAction::Read] {
        assert_eq!(
            SaaSGuard::check_tenant(&CrudState::<&str>::NonExistent, &caller_a(), action, &p),
            Err("Entity not found".to_string())
        );
    }
}

#[test]
fn check_tenant_deleted_state() {
    let p = policy();
    assert_eq!(
        SaaSGuard::check_tenant(&deleted(), &caller_a(), CrudAction::Read, &p),
        Ok(())
    );
    assert!(SaaSGuard::check_tenant(&deleted(), &caller_b(), CrudAction::Read, &p).is_err());
}

#[test]
fn check_authorization_roles() {
    let p = policy();
    assert_eq!(
        SaaSGuard::check_authorization(&caller_a(), CrudAction::Update, &p),
        Ok(())
    );
    assert_eq!(
        SaaSGuard::check_authorization(&caller_admin(), CrudAction::Update, &p),
        Ok(())
    );
    let missing =
        SaaSGuard::check_authorization(&caller_read_only(), CrudAction::Update, &p).unwrap_err();
    assert!(missing.contains("write"), "{missing}");
    let missing =
        SaaSGuard::check_authorization(&caller_read_only(), CrudAction::Delete, &p).unwrap_err();
    assert!(missing.contains("admin"), "{missing}");
    assert_eq!(missing, "Missing roles: admin");
}

// --- custom AuthPolicy -------------------------------------------------------

#[derive(Clone)]
struct ApiKeyAuth {
    #[allow(dead_code)]
    key: String,
    tenant: TenantId,
    is_admin: bool,
}

struct ApiKeyPolicy;

impl AuthPolicy<ApiKeyAuth> for ApiKeyPolicy {
    fn tenant_id(&self, auth: &ApiKeyAuth) -> TenantId {
        auth.tenant.clone()
    }

    fn authorize(&self, auth: &ApiKeyAuth, action: CrudAction) -> Result<(), String> {
        if auth.is_admin {
            Ok(())
        } else if action == CrudAction::Delete {
            Err("Admin only".to_string())
        } else {
            Ok(())
        }
    }
}

#[test]
fn custom_auth_policy_tenant_check_and_authorization() {
    let regular = ApiKeyAuth {
        key: "key-1".into(),
        tenant: TenantId::new("tenant-a"),
        is_admin: false,
    };
    let admin = ApiKeyAuth {
        key: "key-2".into(),
        tenant: TenantId::new("tenant-a"),
        is_admin: true,
    };
    assert_eq!(
        SaaSGuard::check_tenant(&active(), &regular, CrudAction::Read, &ApiKeyPolicy),
        Ok(())
    );
    assert_eq!(
        SaaSGuard::check_authorization(&regular, CrudAction::Delete, &ApiKeyPolicy),
        Err("Admin only".to_string())
    );
    assert_eq!(
        SaaSGuard::check_authorization(&admin, CrudAction::Delete, &ApiKeyPolicy),
        Ok(())
    );
    assert_eq!(
        SaaSGuard::check_authorization(&regular, CrudAction::Read, &ApiKeyPolicy),
        Ok(())
    );
    assert_eq!(
        SaaSGuard::check(&active(), &regular, CrudAction::Read, &ApiKeyPolicy),
        Ok(())
    );
}
