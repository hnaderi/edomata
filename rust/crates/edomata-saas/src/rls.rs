//! The Row-Level Security access contract of event-sourced SaaS services:
//! tenant keys, the access policy (roles and tenant setting) and the
//! [`Access`] values that PostgreSQL turns into a role.

use std::fmt;
use std::marker::PhantomData;

use crate::TenantId;

/// The separator between the tenant and the rest of a tenant-scoped
/// stream or command id: `{tenant}/{id}`. The generated `tenant_id` column
/// of the event-sourced tables is the part before the first separator, so
/// a tenant key can never contain it.
pub const TENANT_SEPARATOR: char = '/';

/// A tenant identifier as the database sees it. The tenant id **type** is
/// the one thing a project chooses (a string, a UUID...): implement this
/// trait for it. [`TenantId`], `String` and `&str` already implement it.
///
/// The key is validated by [`RlsAccessPolicy::tenant_access`] (not
/// empty, no `/`, no NUL), so an implementation need not check it.
///
/// ```
/// use edomata_saas::TenantKey;
///
/// struct OrgNumber(u64);
///
/// impl TenantKey for OrgNumber {
///     fn tenant_key(&self) -> String {
///         format!("org{}", self.0)
///     }
/// }
/// assert_eq!(OrgNumber(7).tenant_key(), "org7");
/// ```
pub trait TenantKey {
    /// The text stored in the `tenant_id` column and set in the tenant
    /// setting of the transaction.
    fn tenant_key(&self) -> String;
}

impl TenantKey for TenantId {
    fn tenant_key(&self) -> String {
        self.value().to_owned()
    }
}

impl TenantKey for String {
    fn tenant_key(&self) -> String {
        self.clone()
    }
}

impl TenantKey for &str {
    fn tenant_key(&self) -> String {
        (*self).to_owned()
    }
}

/// Why a tenant key is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TenantKeyError {
    /// The key is empty: an empty tenant denies everything, so it is never
    /// a valid tenant.
    Empty,
    /// The key contains `/`, which separates the tenant from the id in
    /// `{tenant}/{id}`. A tenant `a/b` would be read by the database as
    /// tenant `a`: refused.
    ContainsSeparator,
    /// The key contains a NUL character, which PostgreSQL text cannot hold.
    ContainsNul,
}

impl fmt::Display for TenantKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TenantKeyError::Empty => f.write_str("the tenant id is empty"),
            TenantKeyError::ContainsSeparator => {
                f.write_str("the tenant id contains '/', the separator of `{tenant}/{id}`")
            }
            TenantKeyError::ContainsNul => f.write_str("the tenant id contains a NUL character"),
        }
    }
}

impl std::error::Error for TenantKeyError {}

/// Checks a tenant key: not empty, no `/`, no NUL.
///
/// # Errors
///
/// A [`TenantKeyError`] naming the first problem.
///
/// ```
/// use edomata_saas::{TenantKeyError, validate_tenant_key};
///
/// assert_eq!(validate_tenant_key("acme"), Ok(()));
/// assert_eq!(validate_tenant_key(""), Err(TenantKeyError::Empty));
/// assert_eq!(validate_tenant_key("a/b"), Err(TenantKeyError::ContainsSeparator));
/// ```
pub fn validate_tenant_key(key: &str) -> Result<(), TenantKeyError> {
    if key.is_empty() {
        Err(TenantKeyError::Empty)
    } else if key.contains(TENANT_SEPARATOR) {
        Err(TenantKeyError::ContainsSeparator)
    } else if key.contains('\0') {
        Err(TenantKeyError::ContainsNul)
    } else {
        Ok(())
    }
}

/// Why an [`RlsAccessPolicy`] is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RlsPolicyError {
    /// A role, prefix or login name is not a plain lowercase PostgreSQL
    /// identifier (`[a-z_][a-z0-9_]*`, at most 63 characters).
    InvalidIdentifier(String),
    /// The tenant setting is not `namespace.name` of two such identifiers
    /// (a custom setting needs the dot).
    InvalidSetting(String),
    /// Two of the three roles have the same name.
    DuplicateRole(String),
}

impl fmt::Display for RlsPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RlsPolicyError::InvalidIdentifier(s) => write!(
                f,
                "`{s}` is not a lowercase PostgreSQL identifier ([a-z_][a-z0-9_]*, at most 63 characters)"
            ),
            RlsPolicyError::InvalidSetting(s) => write!(
                f,
                "`{s}` is not a `namespace.name` setting of two lowercase identifiers"
            ),
            RlsPolicyError::DuplicateRole(s) => {
                write!(f, "the role `{s}` is used for two access modes")
            }
        }
    }
}

impl std::error::Error for RlsPolicyError {}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase() || c == '_')
        && s.len() <= 63
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Validates a PostgreSQL role name for use in generated SQL.
///
/// # Errors
///
/// [`RlsPolicyError::InvalidIdentifier`].
pub(crate) fn check_identifier(s: &str) -> Result<(), RlsPolicyError> {
    if is_identifier(s) {
        Ok(())
    } else {
        Err(RlsPolicyError::InvalidIdentifier(s.to_owned()))
    }
}

/// The three ways to reach the event-sourced tables, each a PostgreSQL
/// role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AccessMode {
    /// Reads and writes the rows of one tenant (`USING` and `WITH CHECK`).
    Tenant,
    /// Reads and writes every row: relays, data migrations, system jobs.
    System,
    /// Reads every row, writes none: an inspection agent.
    Inspection,
}

/// The access policy of a SaaS event-sourced service: the PostgreSQL
/// roles of the three [`AccessMode`]s and the name of the transaction-local
/// setting that carries the tenant. Required to build a
/// [`SaaSEventSourcedService`](crate::SaaSEventSourcedService); only the
/// tenant id type `T` is left to the project.
///
/// The roles are `NOLOGIN` roles that own nothing. A connection reaches
/// one of them with `SET LOCAL ROLE`, which PostgreSQL only allows for a
/// role the connection's login is a **member** of: that membership, not a
/// setting the application can freely `SET`, is the boundary in front of
/// the system and inspection modes (see
/// [`SaaSPGSchema::rls_memberships`](crate::SaaSPGSchema::rls_memberships)).
///
/// ```
/// use edomata_saas::{AccessMode, RlsAccessPolicy, TenantId};
///
/// let policy: RlsAccessPolicy<TenantId> = RlsAccessPolicy::with_prefix("shop").unwrap();
/// assert_eq!(policy.tenant_role(), "shop_tenant");
/// assert_eq!(policy.tenant_setting(), "shop.tenant_id");
///
/// let access = policy.tenant_access(&TenantId::new("acme")).unwrap();
/// assert_eq!(access.mode(), AccessMode::Tenant);
/// assert_eq!(access.tenant(), Some("acme"));
/// assert_eq!(policy.scoped_id(&TenantId::new("acme"), "order-1").unwrap(), "acme/order-1");
///
/// // A tenant that contains the separator is refused.
/// assert!(policy.tenant_access(&TenantId::new("acme/evil")).is_err());
/// ```
pub struct RlsAccessPolicy<T = TenantId> {
    tenant_role: String,
    system_role: String,
    inspection_role: String,
    tenant_setting: String,
    tenant_type: PhantomData<fn(&T)>,
}

impl<T> Clone for RlsAccessPolicy<T> {
    fn clone(&self) -> Self {
        Self {
            tenant_role: self.tenant_role.clone(),
            system_role: self.system_role.clone(),
            inspection_role: self.inspection_role.clone(),
            tenant_setting: self.tenant_setting.clone(),
            tenant_type: PhantomData,
        }
    }
}

impl<T> fmt::Debug for RlsAccessPolicy<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RlsAccessPolicy")
            .field("tenant_role", &self.tenant_role)
            .field("system_role", &self.system_role)
            .field("inspection_role", &self.inspection_role)
            .field("tenant_setting", &self.tenant_setting)
            .finish()
    }
}

impl<T> PartialEq for RlsAccessPolicy<T> {
    fn eq(&self, other: &Self) -> bool {
        self.tenant_role == other.tenant_role
            && self.system_role == other.system_role
            && self.inspection_role == other.inspection_role
            && self.tenant_setting == other.tenant_setting
    }
}

impl<T> Eq for RlsAccessPolicy<T> {}

impl<T> RlsAccessPolicy<T> {
    /// A policy from explicit role names and tenant setting
    /// (`namespace.name`, e.g. `app.tenant_id`).
    ///
    /// # Errors
    ///
    /// [`RlsPolicyError`] when a name is not a lowercase identifier, the
    /// setting is not `namespace.name`, or two roles share a name.
    pub fn new(
        tenant_role: impl Into<String>,
        system_role: impl Into<String>,
        inspection_role: impl Into<String>,
        tenant_setting: impl Into<String>,
    ) -> Result<Self, RlsPolicyError> {
        let policy = Self {
            tenant_role: tenant_role.into(),
            system_role: system_role.into(),
            inspection_role: inspection_role.into(),
            tenant_setting: tenant_setting.into(),
            tenant_type: PhantomData,
        };
        for role in [
            &policy.tenant_role,
            &policy.system_role,
            &policy.inspection_role,
        ] {
            check_identifier(role)?;
        }
        match policy.tenant_setting.split_once('.') {
            Some((ns, name)) if is_identifier(ns) && is_identifier(name) => {}
            _ => {
                return Err(RlsPolicyError::InvalidSetting(policy.tenant_setting));
            }
        }
        for (a, b) in [
            (&policy.tenant_role, &policy.system_role),
            (&policy.tenant_role, &policy.inspection_role),
            (&policy.system_role, &policy.inspection_role),
        ] {
            if a == b {
                return Err(RlsPolicyError::DuplicateRole(a.clone()));
            }
        }
        Ok(policy)
    }

    /// Roles `{prefix}_tenant`, `{prefix}_system` and
    /// `{prefix}_inspection`, and setting `{prefix}.tenant_id`.
    ///
    /// # Errors
    ///
    /// [`RlsPolicyError`] when `prefix` makes an invalid name.
    pub fn with_prefix(prefix: &str) -> Result<Self, RlsPolicyError> {
        check_identifier(prefix)?;
        Self::new(
            format!("{prefix}_tenant"),
            format!("{prefix}_system"),
            format!("{prefix}_inspection"),
            format!("{prefix}.tenant_id"),
        )
    }

    /// The same roles and setting, for another tenant id type.
    pub fn retyped<U>(&self) -> RlsAccessPolicy<U> {
        RlsAccessPolicy {
            tenant_role: self.tenant_role.clone(),
            system_role: self.system_role.clone(),
            inspection_role: self.inspection_role.clone(),
            tenant_setting: self.tenant_setting.clone(),
            tenant_type: PhantomData,
        }
    }

    /// The role of [`AccessMode::Tenant`].
    pub fn tenant_role(&self) -> &str {
        &self.tenant_role
    }

    /// The role of [`AccessMode::System`].
    pub fn system_role(&self) -> &str {
        &self.system_role
    }

    /// The role of [`AccessMode::Inspection`].
    pub fn inspection_role(&self) -> &str {
        &self.inspection_role
    }

    /// The role of `mode`.
    pub fn role(&self, mode: AccessMode) -> &str {
        match mode {
            AccessMode::Tenant => &self.tenant_role,
            AccessMode::System => &self.system_role,
            AccessMode::Inspection => &self.inspection_role,
        }
    }

    /// The transaction-local setting that carries the tenant.
    pub fn tenant_setting(&self) -> &str {
        &self.tenant_setting
    }

    /// The system mode (full access). Only usable on a connection whose
    /// login is a member of the system role.
    pub fn system_access(&self) -> Access {
        self.access(AccessMode::System, None)
    }

    /// The inspection mode (read-only across tenants). Only usable on a
    /// connection whose login is a member of the inspection role.
    pub fn inspection_access(&self) -> Access {
        self.access(AccessMode::Inspection, None)
    }

    fn access(&self, mode: AccessMode, tenant: Option<String>) -> Access {
        Access {
            mode,
            role: self.role(mode).to_owned(),
            tenant_setting: self.tenant_setting.clone(),
            tenant,
        }
    }
}

impl<T: TenantKey> RlsAccessPolicy<T> {
    /// The tenant mode for `tenant`.
    ///
    /// # Errors
    ///
    /// [`TenantKeyError`] when the key is empty or contains `/` or NUL.
    pub fn tenant_access(&self, tenant: &T) -> Result<Access, TenantKeyError> {
        let key = tenant.tenant_key();
        validate_tenant_key(&key)?;
        Ok(self.access(AccessMode::Tenant, Some(key)))
    }

    /// The tenant-scoped id `{tenant}/{id}` of a stream or a command: what
    /// the generated `tenant_id` column is derived from.
    ///
    /// # Errors
    ///
    /// [`TenantKeyError`] when the tenant key is invalid.
    pub fn scoped_id(&self, tenant: &T, id: &str) -> Result<String, TenantKeyError> {
        let key = tenant.tenant_key();
        validate_tenant_key(&key)?;
        Ok(format!("{key}{TENANT_SEPARATOR}{id}"))
    }
}

/// One way of reaching the database, produced by an [`RlsAccessPolicy`]
/// (so a tenant is always validated): the role to `SET LOCAL ROLE` to, and
/// the tenant to inject. The values cannot be built any other way.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Access {
    mode: AccessMode,
    role: String,
    tenant_setting: String,
    tenant: Option<String>,
}

impl Access {
    /// The mode.
    pub fn mode(&self) -> AccessMode {
        self.mode
    }

    /// The PostgreSQL role of the mode.
    pub fn role(&self) -> &str {
        &self.role
    }

    /// The tenant (tenant mode only).
    pub fn tenant(&self) -> Option<&str> {
        self.tenant.as_deref()
    }

    /// The name of the transaction-local tenant setting.
    pub fn tenant_setting(&self) -> &str {
        &self.tenant_setting
    }

    /// The value set in the tenant setting: the tenant, or the empty
    /// string (which denies every row) for the system and inspection
    /// modes.
    pub fn tenant_setting_value(&self) -> &str {
        self.tenant.as_deref().unwrap_or("")
    }

    /// The statement that switches the transaction to the role of the
    /// mode. `SET LOCAL` ends with the transaction; PostgreSQL refuses it
    /// unless the login is a member of the role. The role is a validated
    /// identifier, safe to splice.
    pub fn set_role_statement(&self) -> String {
        format!("SET LOCAL ROLE {}", self.role)
    }
}
