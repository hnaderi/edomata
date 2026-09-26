//! Guarded DSLs for event-sourced and CQRS SaaS programs.

use std::future::Future;
use std::sync::Arc;

use edomata_core::{
    CommandMessage, CqrsDomainDsl, Decision, DomainDsl, Edomaton, NonEmpty, RequestContext,
    ResponseD, ResponseE, Stomaton,
};

use crate::{AuthPolicy, CrudAction, CrudState, SaaSCommand, SaaSGuard};

type MkRejection<R> = Arc<dyn Fn(String) -> R + Send + Sync>;

/// The program type produced by [`SaaSDomainDsl`].
pub type SaaSEsApp<Auth, C, A, E, R, N, T> =
    Edomaton<RequestContext<SaaSCommand<Auth, C>, CrudState<A>>, R, E, N, T>;

/// The program type produced by [`SaaSCqrsDsl`].
pub type SaaSCqrsApp<Auth, C, A, R, N, T> =
    Stomaton<CommandMessage<SaaSCommand<Auth, C>>, CrudState<A>, R, N, T>;

/// Guarded DSL for event-sourced SaaS aggregates. Mirrors Scala's
/// `SaaSDomainDSL`: commands are `SaaSCommand<Auth, C>`, states are
/// `CrudState<A>`, and `guarded_router` / `guarded` run the tenant and
/// authorization checks before the business logic. Guard failures are
/// turned into rejections with `mk_rejection`.
pub struct SaaSDomainDsl<Auth, C, A, E, R, N> {
    inner: DomainDsl<SaaSCommand<Auth, C>, CrudState<A>, E, R, N>,
    policy: Arc<dyn AuthPolicy<Auth>>,
    mk_rejection: MkRejection<R>,
}

impl<Auth, C, A, E, R, N> Clone for SaaSDomainDsl<Auth, C, A, E, R, N> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner,
            policy: Arc::clone(&self.policy),
            mk_rejection: Arc::clone(&self.mk_rejection),
        }
    }
}

impl<Auth, C, A, E, R, N> std::fmt::Debug for SaaSDomainDsl<Auth, C, A, E, R, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SaaSDomainDsl")
    }
}

impl<Auth, C, A, E, R, N> SaaSDomainDsl<Auth, C, A, E, R, N>
where
    Auth: Clone + Send + Sync + 'static,
    C: Clone + Send + Sync + 'static,
    A: Clone + Send + Sync + 'static,
    E: Clone + Send + Sync + 'static,
    R: Clone + Send + Sync + 'static,
    N: Clone + Send + Sync + 'static,
{
    /// Builds the DSL from a policy and a function turning guard messages
    /// (`"Tenant mismatch"`, `"Entity not found"`, authorization reasons)
    /// into domain rejections.
    pub fn new<P, F>(policy: P, mk_rejection: F) -> Self
    where
        P: AuthPolicy<Auth> + 'static,
        F: Fn(String) -> R + Send + Sync + 'static,
    {
        Self {
            inner: DomainDsl::new(),
            policy: Arc::new(policy),
            mk_rejection: Arc::new(mk_rejection),
        }
    }

    /// The raw core DSL marker (for `Backend::builder`).
    pub fn domain(&self) -> DomainDsl<SaaSCommand<Auth, C>, CrudState<A>, E, R, N> {
        self.inner
    }

    /// The policy in use.
    pub fn policy(&self) -> &Arc<dyn AuthPolicy<Auth>> {
        &self.policy
    }

    /// The caller's authentication context.
    pub fn auth(&self) -> SaaSEsApp<Auth, C, A, E, R, N, Auth> {
        self.inner.command().map(|c| c.auth)
    }

    /// The business command.
    pub fn command(&self) -> SaaSEsApp<Auth, C, A, E, R, N, C> {
        self.inner.command().map(|c| c.payload)
    }

    /// The entity's lifecycle state.
    pub fn entity_state(&self) -> SaaSEsApp<Auth, C, A, E, R, N, CrudState<A>> {
        self.inner.state()
    }

    /// The tenant and authorization checks for `action`, as a program.
    pub fn guard(&self, action: CrudAction) -> SaaSEsApp<Auth, C, A, E, R, N, ()> {
        let policy = Arc::clone(&self.policy);
        let mk = Arc::clone(&self.mk_rejection);
        Edomaton::from_fn(
            move |ctx: RequestContext<SaaSCommand<Auth, C>, CrudState<A>>| match SaaSGuard::check(
                &ctx.state,
                &ctx.command.payload.auth,
                action,
                &*policy,
            ) {
                Ok(()) => ResponseD::unit(),
                Err(msg) => ResponseD::reject(mk(msg)),
            },
        )
    }

    /// Routes each command to a `(action, logic)` pair; the guard for
    /// `action` runs before `logic`.
    pub fn guarded_router<F>(&self, f: F) -> SaaSEsApp<Auth, C, A, E, R, N, ()>
    where
        F: Fn(C) -> (CrudAction, SaaSEsApp<Auth, C, A, E, R, N, ()>) + Send + Sync + 'static,
    {
        let this = self.clone();
        self.command().and_then(move |cmd| {
            let (action, logic) = f(cmd);
            this.guard(action).then(logic)
        })
    }

    /// Routes each command without any check. For administrative use only.
    pub fn unsafe_unguarded_router<F>(&self, f: F) -> SaaSEsApp<Auth, C, A, E, R, N, ()>
    where
        F: Fn(C) -> SaaSEsApp<Auth, C, A, E, R, N, ()> + Send + Sync + 'static,
    {
        self.command().and_then(f)
    }

    /// Runs the guard for `action`, then `logic`.
    pub fn guarded(
        &self,
        action: CrudAction,
        logic: SaaSEsApp<Auth, C, A, E, R, N, ()>,
    ) -> SaaSEsApp<Auth, C, A, E, R, N, ()> {
        self.guard(action).then(logic)
    }

    /// Runs `logic` without any check. For administrative use only.
    pub fn unsafe_unguarded(
        &self,
        logic: SaaSEsApp<Auth, C, A, E, R, N, ()>,
    ) -> SaaSEsApp<Auth, C, A, E, R, N, ()> {
        logic
    }

    /// A program that decides the given decision.
    pub fn decide<T: Clone + Send + Sync + 'static>(
        &self,
        d: Decision<R, E, T>,
    ) -> SaaSEsApp<Auth, C, A, E, R, N, T> {
        self.inner.decide(d)
    }

    /// A program that accepts events.
    pub fn accept(&self, events: impl Into<NonEmpty<E>>) -> SaaSEsApp<Auth, C, A, E, R, N, ()> {
        self.inner.accept(events)
    }

    /// A program that rejects.
    pub fn reject<T: Send + 'static>(
        &self,
        reasons: impl Into<NonEmpty<R>>,
    ) -> SaaSEsApp<Auth, C, A, E, R, N, T> {
        self.inner.reject(reasons)
    }

    /// A program that publishes notifications.
    pub fn publish<I>(&self, ns: I) -> SaaSEsApp<Auth, C, A, E, R, N, ()>
    where
        I: IntoIterator<Item = N> + Clone + Send + Sync + 'static,
    {
        self.inner.publish(ns)
    }

    /// A program that evaluates an effect.
    pub fn eval<T, F, Fut>(&self, f: F) -> SaaSEsApp<Auth, C, A, E, R, N, T>
    where
        T: Send + 'static,
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = T> + Send + 'static,
    {
        self.inner.eval(f)
    }

    /// A program that outputs a pure value.
    pub fn pure<T: Clone + Send + Sync + 'static>(
        &self,
        t: T,
    ) -> SaaSEsApp<Auth, C, A, E, R, N, T> {
        self.inner.pure(t)
    }

    /// A program with a trivial output.
    pub fn unit(&self) -> SaaSEsApp<Auth, C, A, E, R, N, ()> {
        self.inner.unit()
    }

    /// The entity's address.
    pub fn aggregate_id(&self) -> SaaSEsApp<Auth, C, A, E, R, N, String> {
        self.inner.aggregate_id()
    }

    /// A program from a validation.
    pub fn validate<T: Clone + Send + Sync + 'static>(
        &self,
        v: Result<T, NonEmpty<R>>,
    ) -> SaaSEsApp<Auth, C, A, E, R, N, T> {
        self.inner.validate(v)
    }
}

/// Guarded DSL for CQRS SaaS aggregates. Mirrors Scala's
/// `SaaSCQRSDomainDSL`.
pub struct SaaSCqrsDsl<Auth, C, A, R, N> {
    inner: CqrsDomainDsl<SaaSCommand<Auth, C>, CrudState<A>, R, N>,
    policy: Arc<dyn AuthPolicy<Auth>>,
    mk_rejection: MkRejection<R>,
}

impl<Auth, C, A, R, N> Clone for SaaSCqrsDsl<Auth, C, A, R, N> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner,
            policy: Arc::clone(&self.policy),
            mk_rejection: Arc::clone(&self.mk_rejection),
        }
    }
}

impl<Auth, C, A, R, N> std::fmt::Debug for SaaSCqrsDsl<Auth, C, A, R, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SaaSCqrsDsl")
    }
}

impl<Auth, C, A, R, N> SaaSCqrsDsl<Auth, C, A, R, N>
where
    Auth: Clone + Send + Sync + 'static,
    C: Clone + Send + Sync + 'static,
    A: Clone + Send + Sync + 'static,
    R: Clone + Send + Sync + 'static,
    N: Clone + Send + Sync + 'static,
{
    /// Builds the DSL from a policy and a function turning guard messages
    /// into domain rejections.
    pub fn new<P, F>(policy: P, mk_rejection: F) -> Self
    where
        P: AuthPolicy<Auth> + 'static,
        F: Fn(String) -> R + Send + Sync + 'static,
    {
        Self {
            inner: CqrsDomainDsl::new(),
            policy: Arc::new(policy),
            mk_rejection: Arc::new(mk_rejection),
        }
    }

    /// The raw core DSL marker (for `cqrs::Backend::builder`).
    pub fn domain(&self) -> CqrsDomainDsl<SaaSCommand<Auth, C>, CrudState<A>, R, N> {
        self.inner
    }

    /// The policy in use.
    pub fn policy(&self) -> &Arc<dyn AuthPolicy<Auth>> {
        &self.policy
    }

    /// The caller's authentication context.
    pub fn auth(&self) -> SaaSCqrsApp<Auth, C, A, R, N, Auth> {
        self.inner.command().map(|c| c.auth)
    }

    /// The business command.
    pub fn command(&self) -> SaaSCqrsApp<Auth, C, A, R, N, C> {
        self.inner.command().map(|c| c.payload)
    }

    /// The entity's lifecycle state.
    pub fn entity_state(&self) -> SaaSCqrsApp<Auth, C, A, R, N, CrudState<A>> {
        self.inner.state()
    }

    /// The tenant and authorization checks for `action`, as a program.
    pub fn guard(&self, action: CrudAction) -> SaaSCqrsApp<Auth, C, A, R, N, ()> {
        let policy = Arc::clone(&self.policy);
        let mk = Arc::clone(&self.mk_rejection);
        Stomaton::from_fn(
            move |cmd: CommandMessage<SaaSCommand<Auth, C>>, state: CrudState<A>| {
                match SaaSGuard::check(&state, &cmd.payload.auth, action, &*policy) {
                    Ok(()) => ResponseE::pure((state, ())),
                    Err(msg) => ResponseE::reject(mk(msg)),
                }
            },
        )
    }

    /// Routes each command to a `(action, logic)` pair; the guard for
    /// `action` runs before `logic`.
    pub fn guarded_router<F>(&self, f: F) -> SaaSCqrsApp<Auth, C, A, R, N, ()>
    where
        F: Fn(C) -> (CrudAction, SaaSCqrsApp<Auth, C, A, R, N, ()>) + Send + Sync + 'static,
    {
        let this = self.clone();
        self.command().and_then(move |cmd| {
            let (action, logic) = f(cmd);
            this.guard(action).then(logic)
        })
    }

    /// Routes each command without any check. For administrative use only.
    pub fn unsafe_unguarded_router<F>(&self, f: F) -> SaaSCqrsApp<Auth, C, A, R, N, ()>
    where
        F: Fn(C) -> SaaSCqrsApp<Auth, C, A, R, N, ()> + Send + Sync + 'static,
    {
        self.command().and_then(f)
    }

    /// Runs the guard for `action`, then `logic`.
    pub fn guarded(
        &self,
        action: CrudAction,
        logic: SaaSCqrsApp<Auth, C, A, R, N, ()>,
    ) -> SaaSCqrsApp<Auth, C, A, R, N, ()> {
        self.guard(action).then(logic)
    }

    /// Runs `logic` without any check. For administrative use only.
    pub fn unsafe_unguarded(
        &self,
        logic: SaaSCqrsApp<Auth, C, A, R, N, ()>,
    ) -> SaaSCqrsApp<Auth, C, A, R, N, ()> {
        logic
    }

    /// Sets the entity state.
    pub fn set(&self, s: CrudState<A>) -> SaaSCqrsApp<Auth, C, A, R, N, ()> {
        self.inner.set(s)
    }

    /// Decides on a new state from the current one and outputs it.
    pub fn modify_s<F>(&self, f: F) -> SaaSCqrsApp<Auth, C, A, R, N, CrudState<A>>
    where
        F: Fn(CrudState<A>) -> Result<CrudState<A>, NonEmpty<R>> + Send + Sync + 'static,
    {
        self.inner.modify_s(f)
    }

    /// Alias of [`modify_s`](Self::modify_s).
    pub fn decide_s<F>(&self, f: F) -> SaaSCqrsApp<Auth, C, A, R, N, CrudState<A>>
    where
        F: Fn(CrudState<A>) -> Result<CrudState<A>, NonEmpty<R>> + Send + Sync + 'static,
    {
        self.inner.decide_s(f)
    }

    /// A program that rejects.
    pub fn reject<T: Send + 'static>(
        &self,
        reasons: impl Into<NonEmpty<R>>,
    ) -> SaaSCqrsApp<Auth, C, A, R, N, T> {
        self.inner.reject(reasons)
    }

    /// A program that publishes notifications.
    pub fn publish<I>(&self, ns: I) -> SaaSCqrsApp<Auth, C, A, R, N, ()>
    where
        I: IntoIterator<Item = N> + Clone + Send + Sync + 'static,
    {
        self.inner.publish(ns)
    }

    /// A program that evaluates an effect.
    pub fn eval<T, F, Fut>(&self, f: F) -> SaaSCqrsApp<Auth, C, A, R, N, T>
    where
        T: Send + 'static,
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = T> + Send + 'static,
    {
        self.inner.eval(f)
    }

    /// A program that outputs a pure value.
    pub fn pure<T: Clone + Send + Sync + 'static>(&self, t: T) -> SaaSCqrsApp<Auth, C, A, R, N, T> {
        self.inner.pure(t)
    }

    /// A program with a trivial output.
    pub fn unit(&self) -> SaaSCqrsApp<Auth, C, A, R, N, ()> {
        self.inner.unit()
    }

    /// The entity's address.
    pub fn aggregate_id(&self) -> SaaSCqrsApp<Auth, C, A, R, N, String> {
        self.inner.aggregate_id()
    }

    /// A program from a validation.
    pub fn validate<T: Clone + Send + Sync + 'static>(
        &self,
        v: Result<T, NonEmpty<R>>,
    ) -> SaaSCqrsApp<Auth, C, A, R, N, T> {
        self.inner.validate(v)
    }
}
