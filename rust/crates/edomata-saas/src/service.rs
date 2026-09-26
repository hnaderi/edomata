//! Service bundles: the guarded DSL plus the domain marker for backends.

use edomata_core::{CqrsDomainDsl, DomainDsl};

use crate::{AuthPolicy, CrudState, SaaSCommand, SaaSCqrsDsl, SaaSDomainDsl};

/// An event-sourced SaaS service: holds the guarded [`SaaSDomainDsl`] and
/// exposes the domain marker needed by `Backend::builder`. Mirrors Scala's
/// `SaaSEventSourcedService`, where only the guarded `SaaS` DSL is reachable.
pub struct SaaSEventSourcedService<Auth, C, A, E, R, N> {
    saas: SaaSDomainDsl<Auth, C, A, E, R, N>,
}

impl<Auth, C, A, E, R, N> SaaSEventSourcedService<Auth, C, A, E, R, N>
where
    Auth: Clone + Send + Sync + 'static,
    C: Clone + Send + Sync + 'static,
    A: Clone + Send + Sync + 'static,
    E: Clone + Send + Sync + 'static,
    R: Clone + Send + Sync + 'static,
    N: Clone + Send + Sync + 'static,
{
    /// Builds the service.
    pub fn new<P, F>(policy: P, mk_rejection: F) -> Self
    where
        P: AuthPolicy<Auth> + 'static,
        F: Fn(String) -> R + Send + Sync + 'static,
    {
        Self {
            saas: SaaSDomainDsl::new(policy, mk_rejection),
        }
    }

    /// The guarded DSL.
    pub fn saas(&self) -> &SaaSDomainDsl<Auth, C, A, E, R, N> {
        &self.saas
    }

    /// The domain marker for `Backend::builder`.
    pub fn domain(&self) -> DomainDsl<SaaSCommand<Auth, C>, CrudState<A>, E, R, N> {
        self.saas.domain()
    }
}

/// A CQRS SaaS service: holds the guarded [`SaaSCqrsDsl`] and exposes the
/// domain marker needed by `cqrs::Backend::builder`. Mirrors Scala's
/// `SaaSCQRSService`.
pub struct SaaSCqrsService<Auth, C, A, R, N> {
    saas: SaaSCqrsDsl<Auth, C, A, R, N>,
}

impl<Auth, C, A, R, N> SaaSCqrsService<Auth, C, A, R, N>
where
    Auth: Clone + Send + Sync + 'static,
    C: Clone + Send + Sync + 'static,
    A: Clone + Send + Sync + 'static,
    R: Clone + Send + Sync + 'static,
    N: Clone + Send + Sync + 'static,
{
    /// Builds the service.
    pub fn new<P, F>(policy: P, mk_rejection: F) -> Self
    where
        P: AuthPolicy<Auth> + 'static,
        F: Fn(String) -> R + Send + Sync + 'static,
    {
        Self {
            saas: SaaSCqrsDsl::new(policy, mk_rejection),
        }
    }

    /// The guarded DSL.
    pub fn saas(&self) -> &SaaSCqrsDsl<Auth, C, A, R, N> {
        &self.saas
    }

    /// The domain marker for `cqrs::Backend::builder`.
    pub fn domain(&self) -> CqrsDomainDsl<SaaSCommand<Auth, C>, CrudState<A>, R, N> {
        self.saas.domain()
    }
}
