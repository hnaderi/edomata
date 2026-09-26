//! Tenancy types and authorization policies.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

/// Identifier of a tenant. Any string is accepted, as in Scala.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(transparent)
)]
pub struct TenantId(String);

/// Identifier of a user. Any string is accepted, as in Scala.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(transparent)
)]
pub struct UserId(String);

macro_rules! string_id {
    ($t:ident) => {
        impl $t {
            /// Wraps a string.
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            /// The underlying string.
            pub fn value(&self) -> &str {
                &self.0
            }

            /// Consumes the id, returning the string.
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl From<&str> for $t {
            fn from(s: &str) -> Self {
                Self(s.to_owned())
            }
        }

        impl From<String> for $t {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl From<$t> for String {
            fn from(id: $t) -> String {
                id.0
            }
        }

        impl AsRef<str> for $t {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

string_id!(TenantId);
string_id!(UserId);

/// The kind of operation a command performs on an entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CrudAction {
    /// Creates the entity.
    Create,
    /// Reads the entity.
    Read,
    /// Updates the entity.
    Update,
    /// Deletes the entity.
    Delete,
}

impl CrudAction {
    /// All actions, in declaration order.
    pub const ALL: [CrudAction; 4] = [
        CrudAction::Create,
        CrudAction::Read,
        CrudAction::Update,
        CrudAction::Delete,
    ];
}

/// Lifecycle state of a tenant-owned entity.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CrudState<A> {
    /// The entity does not exist (yet).
    #[default]
    NonExistent,
    /// The entity exists and belongs to a tenant and an owner.
    Active {
        /// Owning tenant.
        tenant_id: TenantId,
        /// Owning user.
        owner_id: UserId,
        /// Business data.
        data: A,
    },
    /// The entity was deleted; tenant and owner are kept for auditing.
    Deleted {
        /// Owning tenant.
        tenant_id: TenantId,
        /// Owning user.
        owner_id: UserId,
    },
}

impl<A> CrudState<A> {
    /// An active entity.
    pub fn active(tenant_id: impl Into<TenantId>, owner_id: impl Into<UserId>, data: A) -> Self {
        CrudState::Active {
            tenant_id: tenant_id.into(),
            owner_id: owner_id.into(),
            data,
        }
    }

    /// A deleted entity.
    pub fn deleted(tenant_id: impl Into<TenantId>, owner_id: impl Into<UserId>) -> Self {
        CrudState::Deleted {
            tenant_id: tenant_id.into(),
            owner_id: owner_id.into(),
        }
    }

    /// The tenant, unless the entity does not exist.
    pub fn tenant_id(&self) -> Option<&TenantId> {
        match self {
            CrudState::NonExistent => None,
            CrudState::Active { tenant_id, .. } | CrudState::Deleted { tenant_id, .. } => {
                Some(tenant_id)
            }
        }
    }

    /// The owner, unless the entity does not exist.
    pub fn owner_id(&self) -> Option<&UserId> {
        match self {
            CrudState::NonExistent => None,
            CrudState::Active { owner_id, .. } | CrudState::Deleted { owner_id, .. } => {
                Some(owner_id)
            }
        }
    }

    /// The business data of an active entity.
    pub fn data(&self) -> Option<&A> {
        match self {
            CrudState::Active { data, .. } => Some(data),
            _ => None,
        }
    }

    /// Whether the entity is active.
    pub fn is_active(&self) -> bool {
        matches!(self, CrudState::Active { .. })
    }

    /// Changes the business data, keeping the lifecycle state.
    pub fn map<B, F: FnOnce(A) -> B>(self, f: F) -> CrudState<B> {
        match self {
            CrudState::NonExistent => CrudState::NonExistent,
            CrudState::Active {
                tenant_id,
                owner_id,
                data,
            } => CrudState::Active {
                tenant_id,
                owner_id,
                data: f(data),
            },
            CrudState::Deleted {
                tenant_id,
                owner_id,
            } => CrudState::Deleted {
                tenant_id,
                owner_id,
            },
        }
    }
}

/// A business command together with the caller's authentication context.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SaaSCommand<Auth, C> {
    /// Who issues the command.
    pub auth: Auth,
    /// The business command.
    pub payload: C,
}

impl<Auth, C> SaaSCommand<Auth, C> {
    /// Pairs an auth context with a command.
    pub fn new(auth: Auth, payload: C) -> Self {
        Self { auth, payload }
    }
}

/// Extracts the tenant from an authentication context and decides whether
/// it may perform an action. Implement it for your own auth type (JWT
/// claims, API key context, ...).
pub trait AuthPolicy<Auth>: Send + Sync {
    /// The tenant the caller acts for.
    fn tenant_id(&self, auth: &Auth) -> TenantId;

    /// `Ok(())` if the caller may perform `action`, `Err(reason)` otherwise.
    fn authorize(&self, auth: &Auth, action: CrudAction) -> Result<(), String>;
}

impl<Auth, P: AuthPolicy<Auth> + ?Sized> AuthPolicy<Auth> for Arc<P> {
    fn tenant_id(&self, auth: &Auth) -> TenantId {
        (**self).tenant_id(auth)
    }

    fn authorize(&self, auth: &Auth, action: CrudAction) -> Result<(), String> {
        (**self).authorize(auth, action)
    }
}

/// A convenient default authentication context: tenant, user and roles.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CallerIdentity {
    /// The caller's tenant.
    pub tenant_id: TenantId,
    /// The caller.
    pub user_id: UserId,
    /// Roles granted to the caller.
    pub roles: BTreeSet<String>,
}

impl CallerIdentity {
    /// Builds an identity.
    pub fn new(
        tenant_id: impl Into<TenantId>,
        user_id: impl Into<UserId>,
        roles: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            user_id: user_id.into(),
            roles: roles.into_iter().map(Into::into).collect(),
        }
    }
}

/// The default policy for [`CallerIdentity`] (Scala's `given
/// AuthPolicy[CallerIdentity]`): every action is authorized, only tenant
/// isolation applies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PermissivePolicy;

impl AuthPolicy<CallerIdentity> for PermissivePolicy {
    fn tenant_id(&self, auth: &CallerIdentity) -> TenantId {
        auth.tenant_id.clone()
    }

    fn authorize(&self, _auth: &CallerIdentity, _action: CrudAction) -> Result<(), String> {
        Ok(())
    }
}

/// A role-based policy for [`CallerIdentity`]: each action requires a set
/// of roles; missing roles are reported as `"Missing roles: a, b"`.
///
/// ```
/// use edomata_saas::{AuthPolicy, CallerIdentity, CrudAction, RoleBasedPolicy};
///
/// let policy = RoleBasedPolicy::new(|action| match action {
///     CrudAction::Delete => ["admin"].into_iter().map(String::from).collect(),
///     _ => ["write"].into_iter().map(String::from).collect(),
/// });
/// let writer = CallerIdentity::new("t", "u", ["write"]);
/// assert_eq!(policy.authorize(&writer, CrudAction::Update), Ok(()));
/// assert_eq!(policy.authorize(&writer, CrudAction::Delete), Err("Missing roles: admin".to_string()));
/// ```
#[derive(Clone)]
pub struct RoleBasedPolicy {
    roles_for: Arc<dyn Fn(CrudAction) -> BTreeSet<String> + Send + Sync>,
}

impl fmt::Debug for RoleBasedPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RoleBasedPolicy")
    }
}

impl RoleBasedPolicy {
    /// A policy requiring `roles_for(action)` for each action.
    pub fn new<F>(roles_for: F) -> Self
    where
        F: Fn(CrudAction) -> BTreeSet<String> + Send + Sync + 'static,
    {
        Self {
            roles_for: Arc::new(roles_for),
        }
    }

    /// A policy requiring no role at all (tenant isolation only).
    pub fn none() -> Self {
        Self::new(|_| BTreeSet::new())
    }

    /// The roles required for `action`.
    pub fn roles_for(&self, action: CrudAction) -> BTreeSet<String> {
        (self.roles_for)(action)
    }
}

impl AuthPolicy<CallerIdentity> for RoleBasedPolicy {
    fn tenant_id(&self, auth: &CallerIdentity) -> TenantId {
        auth.tenant_id.clone()
    }

    fn authorize(&self, auth: &CallerIdentity, action: CrudAction) -> Result<(), String> {
        let required = self.roles_for(action);
        let missing: Vec<&str> = required
            .iter()
            .filter(|r| !auth.roles.contains(*r))
            .map(String::as_str)
            .collect();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(format!("Missing roles: {}", missing.join(", ")))
        }
    }
}
