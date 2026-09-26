//! Tenant isolation and authorization checks.

use crate::{AuthPolicy, CrudAction, CrudState};

/// The checks every guarded command goes through. Mirrors Scala's
/// `SaaSGuard`; the messages are identical.
///
/// ```
/// use edomata_saas::{CallerIdentity, CrudAction, CrudState, PermissivePolicy, SaaSGuard};
///
/// let state: CrudState<&str> = CrudState::active("tenant-a", "user-a", "data");
/// let same = CallerIdentity::new("tenant-a", "u", Vec::<String>::new());
/// let other = CallerIdentity::new("tenant-b", "u", Vec::<String>::new());
/// assert_eq!(SaaSGuard::check_tenant(&state, &same, CrudAction::Update, &PermissivePolicy), Ok(()));
/// assert_eq!(
///     SaaSGuard::check_tenant(&state, &other, CrudAction::Update, &PermissivePolicy),
///     Err("Tenant mismatch".to_string())
/// );
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct SaaSGuard;

impl SaaSGuard {
    /// Checks that the caller belongs to the entity's tenant. `Create`
    /// always passes (there is no entity yet); a `NonExistent` entity fails
    /// with `"Entity not found"` and a foreign tenant with `"Tenant
    /// mismatch"`.
    pub fn check_tenant<Auth, A, P>(
        state: &CrudState<A>,
        auth: &Auth,
        action: CrudAction,
        policy: &P,
    ) -> Result<(), String>
    where
        P: AuthPolicy<Auth> + ?Sized,
    {
        if action == CrudAction::Create {
            return Ok(());
        }
        match state.tenant_id() {
            None => Err("Entity not found".to_string()),
            Some(tid) if *tid == policy.tenant_id(auth) => Ok(()),
            Some(_) => Err("Tenant mismatch".to_string()),
        }
    }

    /// Delegates to the policy's authorization rule.
    pub fn check_authorization<Auth, P>(
        auth: &Auth,
        action: CrudAction,
        policy: &P,
    ) -> Result<(), String>
    where
        P: AuthPolicy<Auth> + ?Sized,
    {
        policy.authorize(auth, action)
    }

    /// Both checks, tenant first.
    pub fn check<Auth, A, P>(
        state: &CrudState<A>,
        auth: &Auth,
        action: CrudAction,
        policy: &P,
    ) -> Result<(), String>
    where
        P: AuthPolicy<Auth> + ?Sized,
    {
        Self::check_tenant(state, auth, action, policy)?;
        Self::check_authorization(auth, action, policy)
    }
}
