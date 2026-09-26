//! Tenant extraction for tenant-aware storage.

use crate::{CrudState, TenantId, UserId};

/// Extracts tenant and owner from a state so that tenant-aware drivers can
/// populate `tenant_id` / `owner_id` columns. Mirrors Scala's
/// `TenantExtractor[S]` typeclass, implemented on the state type itself.
pub trait TenantExtractor {
    /// The tenant and owner of the entity, if it exists.
    fn tenant_and_owner(&self) -> Option<(TenantId, UserId)>;
}

impl<A> TenantExtractor for CrudState<A> {
    fn tenant_and_owner(&self) -> Option<(TenantId, UserId)> {
        match self {
            CrudState::Active {
                tenant_id,
                owner_id,
                ..
            }
            | CrudState::Deleted {
                tenant_id,
                owner_id,
            } => Some((tenant_id.clone(), owner_id.clone())),
            CrudState::NonExistent => None,
        }
    }
}
