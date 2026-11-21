//! Service bundles: the guarded DSL plus the domain marker for backends.

use std::marker::PhantomData;

use edomata_core::{CqrsDomainDsl, DomainDsl};

use crate::{
    AuthPolicy, CrudState, RlsAccessPolicy, SaaSCommand, SaaSCqrsDsl, SaaSDomainDsl, TenantId,
};

/// A builder slot that has not been filled yet (see
/// [`SaaSEventSourcedServiceBuilder`]).
#[derive(Clone, Copy, Debug)]
pub struct Missing;

/// A builder slot that has been filled with a value.
#[derive(Clone, Copy, Debug)]
pub struct Set<V>(V);

/// An event-sourced SaaS service: holds the guarded [`SaaSDomainDsl`], the
/// mandatory [`RlsAccessPolicy`], and exposes the domain marker needed by
/// `Backend::builder`. Mirrors Scala's `SaaSEventSourcedService`, where only
/// the guarded `SaaS` DSL is reachable.
///
/// **It cannot be built without an access policy**: the only constructor is
/// [`builder`](Self::builder), a typestate builder whose `build` exists once
/// the authorization policy, the rejection mapping *and* the access policy
/// are set. The tenant id type `T` (default [`TenantId`]) is the one thing
/// left to the project.
///
/// ```
/// use edomata_saas::*;
///
/// type Service = SaaSEventSourcedService<CallerIdentity, String, String, String, String, String>;
///
/// let service: Service = SaaSEventSourcedService::builder()
///     .auth_policy(PermissivePolicy)
///     .rejection(|reason| reason)
///     .access_policy(RlsAccessPolicy::with_prefix("shop").unwrap())
///     .build();
/// assert_eq!(service.access_policy().tenant_role(), "shop_tenant");
/// ```
///
/// Leaving the access policy out is a compile error:
///
/// ```compile_fail
/// use edomata_saas::*;
///
/// type Service = SaaSEventSourcedService<CallerIdentity, String, String, String, String, String>;
///
/// // error[E0599]: no method named `build` found: `access_policy` was never called
/// let service: Service = SaaSEventSourcedService::builder()
///     .auth_policy(PermissivePolicy)
///     .rejection(|reason| reason)
///     .build();
/// ```
///
/// There is no `new` that skips it either:
///
/// ```compile_fail
/// use edomata_saas::*;
///
/// type Service = SaaSEventSourcedService<CallerIdentity, String, String, String, String, String>;
///
/// // error[E0599]: no function or associated item named `new` found
/// let service: Service = SaaSEventSourcedService::new(PermissivePolicy, |reason: String| reason);
/// ```
pub struct SaaSEventSourcedService<Auth, C, A, E, R, N, T = TenantId> {
    saas: SaaSDomainDsl<Auth, C, A, E, R, N>,
    access: RlsAccessPolicy<T>,
}

/// The typestate builder of [`SaaSEventSourcedService`]: `build` is only
/// available when the authorization policy, the rejection mapping and the
/// access policy have all been set (their slots are [`Set`], not
/// [`Missing`]).
#[allow(clippy::type_complexity)] // the PhantomData names the service's type parameters
pub struct SaaSEventSourcedServiceBuilder<Auth, C, A, E, R, N, T, P, F, X> {
    auth_policy: P,
    rejection: F,
    access: X,
    types: PhantomData<fn() -> (Auth, C, A, E, R, N, T)>,
}

impl<Auth, C, A, E, R, N, T> SaaSEventSourcedService<Auth, C, A, E, R, N, T> {
    /// Starts building the service: set the
    /// [`auth_policy`](SaaSEventSourcedServiceBuilder::auth_policy), the
    /// [`rejection`](SaaSEventSourcedServiceBuilder::rejection) mapping and
    /// the [`access_policy`](SaaSEventSourcedServiceBuilder::access_policy),
    /// then `build`.
    pub fn builder()
    -> SaaSEventSourcedServiceBuilder<Auth, C, A, E, R, N, T, Missing, Missing, Missing> {
        SaaSEventSourcedServiceBuilder {
            auth_policy: Missing,
            rejection: Missing,
            access: Missing,
            types: PhantomData,
        }
    }
}

impl<Auth, C, A, E, R, N, T, P, F, X>
    SaaSEventSourcedServiceBuilder<Auth, C, A, E, R, N, T, P, F, X>
{
    /// The authorization policy (who may do what: [`AuthPolicy`]).
    pub fn auth_policy<P2>(
        self,
        policy: P2,
    ) -> SaaSEventSourcedServiceBuilder<Auth, C, A, E, R, N, T, Set<P2>, F, X>
    where
        P2: AuthPolicy<Auth> + 'static,
    {
        SaaSEventSourcedServiceBuilder {
            auth_policy: Set(policy),
            rejection: self.rejection,
            access: self.access,
            types: PhantomData,
        }
    }

    /// The function turning guard messages into domain rejections.
    pub fn rejection<F2>(
        self,
        mk_rejection: F2,
    ) -> SaaSEventSourcedServiceBuilder<Auth, C, A, E, R, N, T, P, Set<F2>, X>
    where
        F2: Fn(String) -> R + Send + Sync + 'static,
    {
        SaaSEventSourcedServiceBuilder {
            auth_policy: self.auth_policy,
            rejection: Set(mk_rejection),
            access: self.access,
            types: PhantomData,
        }
    }

    /// The mandatory Row-Level Security access policy.
    #[allow(clippy::type_complexity)] // the typestate is the point
    pub fn access_policy(
        self,
        policy: RlsAccessPolicy<T>,
    ) -> SaaSEventSourcedServiceBuilder<Auth, C, A, E, R, N, T, P, F, Set<RlsAccessPolicy<T>>> {
        SaaSEventSourcedServiceBuilder {
            auth_policy: self.auth_policy,
            rejection: self.rejection,
            access: Set(policy),
            types: PhantomData,
        }
    }
}

impl<Auth, C, A, E, R, N, T, P, F>
    SaaSEventSourcedServiceBuilder<Auth, C, A, E, R, N, T, Set<P>, Set<F>, Set<RlsAccessPolicy<T>>>
where
    Auth: Clone + Send + Sync + 'static,
    C: Clone + Send + Sync + 'static,
    A: Clone + Send + Sync + 'static,
    E: Clone + Send + Sync + 'static,
    R: Clone + Send + Sync + 'static,
    N: Clone + Send + Sync + 'static,
    P: AuthPolicy<Auth> + 'static,
    F: Fn(String) -> R + Send + Sync + 'static,
{
    /// Builds the service. Only callable once all three parts are set.
    pub fn build(self) -> SaaSEventSourcedService<Auth, C, A, E, R, N, T> {
        SaaSEventSourcedService {
            saas: SaaSDomainDsl::new(self.auth_policy.0, self.rejection.0),
            access: self.access.0,
        }
    }
}

impl<Auth, C, A, E, R, N, T> SaaSEventSourcedService<Auth, C, A, E, R, N, T>
where
    Auth: Clone + Send + Sync + 'static,
    C: Clone + Send + Sync + 'static,
    A: Clone + Send + Sync + 'static,
    E: Clone + Send + Sync + 'static,
    R: Clone + Send + Sync + 'static,
    N: Clone + Send + Sync + 'static,
{
    /// The guarded DSL.
    pub fn saas(&self) -> &SaaSDomainDsl<Auth, C, A, E, R, N> {
        &self.saas
    }

    /// The access policy the service was built with. Give it to the
    /// storage layer (`edomata_saas_sqlx::RlsGateway`) so that the service
    /// runs as the tenant role.
    pub fn access_policy(&self) -> &RlsAccessPolicy<T> {
        &self.access
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
