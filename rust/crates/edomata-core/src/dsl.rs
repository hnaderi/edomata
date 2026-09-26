//! Domain-specific helpers for writing programs on a model.

use std::future::Future;
use std::marker::PhantomData;

use crate::{
    CommandMessage, Decision, Edomaton, MessageMetadata, NonEmpty, RequestContext, ResponseD,
    Stomaton,
};

/// Zero-sized marker that is covariant and `Send + Sync` regardless of `T`.
type Tag<T> = PhantomData<fn() -> T>;

/// Helpers for writing [`Edomaton`] programs of a specific event-sourced
/// domain: commands `C`, state `S`, events `E`, rejections `R` and
/// notifications `N`.
///
/// Every method returns an `Edomaton<RequestContext<C, S>, R, E, N, T>`,
/// which fixes the type parameters so that inference works without
/// annotations. Obtain it with [`DomainModel::dsl`](crate::DomainModel::dsl)
/// or [`DomainDsl::new`].
pub struct DomainDsl<C, S, E, R, N> {
    _marker: Tag<(C, S, E, R, N)>,
}

impl<C, S, E, R, N> Clone for DomainDsl<C, S, E, R, N> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C, S, E, R, N> Copy for DomainDsl<C, S, E, R, N> {}

impl<C, S, E, R, N> Default for DomainDsl<C, S, E, R, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C, S, E, R, N> DomainDsl<C, S, E, R, N> {
    /// Creates the DSL.
    pub const fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<C, S, E, R, N> std::fmt::Debug for DomainDsl<C, S, E, R, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DomainDsl")
    }
}

/// The program type produced by a [`DomainDsl`].
pub type App<C, S, E, R, N, T> = Edomaton<RequestContext<C, S>, R, E, N, T>;

impl<C, S, E, R, N> DomainDsl<C, S, E, R, N>
where
    C: Send + 'static,
    S: Send + 'static,
    E: Send + 'static,
    R: Send + 'static,
    N: Send + 'static,
{
    /// A program that outputs a pure value.
    pub fn pure<T: Clone + Send + Sync + 'static>(self, t: T) -> App<C, S, E, R, N, T> {
        Edomaton::pure(t)
    }

    /// A program with a trivial output.
    pub fn unit(self) -> App<C, S, E, R, N, ()> {
        Edomaton::unit()
    }

    /// A program with the given response.
    pub fn lift<T>(self, response: ResponseD<R, E, N, T>) -> App<C, S, E, R, N, T>
    where
        T: Send + 'static,
        ResponseD<R, E, N, T>: Clone + Sync,
    {
        Edomaton::lift(response)
    }

    /// A program that evaluates an effect.
    pub fn eval<T, F, Fut>(self, f: F) -> App<C, S, E, R, N, T>
    where
        T: Send + 'static,
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = T> + Send + 'static,
    {
        Edomaton::eval(f)
    }

    /// A program that runs an effect using the request context.
    pub fn run<T, F, Fut>(self, f: F) -> App<C, S, E, R, N, T>
    where
        T: Send + 'static,
        F: Fn(RequestContext<C, S>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = T> + Send + 'static,
    {
        Edomaton::run_with(f)
    }

    /// A program that outputs the request context.
    pub fn read(self) -> App<C, S, E, R, N, RequestContext<C, S>> {
        Edomaton::read()
    }

    /// A program that publishes notifications.
    pub fn publish<I>(self, ns: I) -> App<C, S, E, R, N, ()>
    where
        I: IntoIterator<Item = N> + Clone + Send + Sync + 'static,
    {
        Edomaton::publish_only(ns)
    }

    /// A program that rejects.
    pub fn reject<T>(self, reasons: impl Into<NonEmpty<R>>) -> App<C, S, E, R, N, T>
    where
        T: Send + 'static,
        R: Clone + Sync,
    {
        Edomaton::reject(reasons)
    }

    /// A program that accepts events.
    pub fn accept(self, events: impl Into<NonEmpty<E>>) -> App<C, S, E, R, N, ()>
    where
        E: Clone + Sync,
    {
        Edomaton::accept(events)
    }

    /// A program that decides the given decision.
    pub fn decide<T>(self, decision: Decision<R, E, T>) -> App<C, S, E, R, N, T>
    where
        T: Send + 'static,
        Decision<R, E, T>: Clone + Sync,
    {
        Edomaton::decide(decision)
    }

    /// A program that decides with a pure function of the request context.
    pub fn decide_with<T, F>(self, f: F) -> App<C, S, E, R, N, T>
    where
        T: Send + 'static,
        F: Fn(RequestContext<C, S>) -> Decision<R, E, T> + Send + Sync + 'static,
    {
        Edomaton::from_fn(move |ctx| ResponseD::lift(f(ctx)))
    }

    /// A program from a validation.
    pub fn validate<T>(self, validation: Result<T, NonEmpty<R>>) -> App<C, S, E, R, N, T>
    where
        T: Send + 'static,
        Decision<R, E, T>: Clone + Sync,
    {
        Edomaton::validate(validation)
    }

    /// A program from an optional value, rejecting when absent.
    pub fn from_option<T>(
        self,
        opt: Option<T>,
        or_else: impl Into<NonEmpty<R>>,
    ) -> App<C, S, E, R, N, T>
    where
        T: Send + 'static,
        Decision<R, E, T>: Clone + Sync,
    {
        Edomaton::from_option(opt, or_else)
    }

    /// A program from a result with a single rejection.
    pub fn from_result<T>(self, result: Result<T, R>) -> App<C, S, E, R, N, T>
    where
        T: Send + 'static,
        Decision<R, E, T>: Clone + Sync,
    {
        Edomaton::from_result(result)
    }

    /// A program from a result with several rejections.
    pub fn from_result_nec<T>(self, result: Result<T, NonEmpty<R>>) -> App<C, S, E, R, N, T>
    where
        T: Send + 'static,
        Decision<R, E, T>: Clone + Sync,
    {
        Edomaton::from_result_nec(result)
    }

    /// A program that outputs the current state.
    pub fn state(self) -> App<C, S, E, R, N, S> {
        Edomaton::map_input(|ctx: RequestContext<C, S>| ctx.state)
    }

    /// A program that outputs the aggregate identifier (the command
    /// address).
    pub fn aggregate_id(self) -> App<C, S, E, R, N, String> {
        Edomaton::map_input(|ctx: RequestContext<C, S>| ctx.command.address)
    }

    /// A program that outputs the command metadata.
    pub fn metadata(self) -> App<C, S, E, R, N, MessageMetadata> {
        Edomaton::map_input(|ctx: RequestContext<C, S>| ctx.command.metadata)
    }

    /// A program that outputs the command message identifier.
    pub fn message_id(self) -> App<C, S, E, R, N, String> {
        Edomaton::map_input(|ctx: RequestContext<C, S>| ctx.command.id)
    }

    /// A program that outputs the command payload.
    pub fn command(self) -> App<C, S, E, R, N, C> {
        Edomaton::map_input(|ctx: RequestContext<C, S>| ctx.command.payload)
    }

    /// A program that outputs the whole command message.
    pub fn command_message(self) -> App<C, S, E, R, N, CommandMessage<C>> {
        Edomaton::map_input(|ctx: RequestContext<C, S>| ctx.command)
    }

    /// Routes the command payload to a program.
    pub fn router<T, F>(self, f: F) -> App<C, S, E, R, N, T>
    where
        C: Clone,
        S: Clone,
        T: Send + 'static,
        F: Fn(C) -> App<C, S, E, R, N, T> + Send + Sync + 'static,
    {
        self.command().and_then(f)
    }
}

/// Helpers for writing [`Stomaton`] programs of a specific CQRS domain:
/// commands `C`, state `S`, rejections `R` and notifications `N`.
///
/// Every method returns a `Stomaton<CommandMessage<C>, S, R, N, T>`. Obtain
/// it with [`CqrsModel::dsl`](crate::CqrsModel::dsl) or
/// [`CqrsDomainDsl::new`].
pub struct CqrsDomainDsl<C, S, R, N> {
    _marker: Tag<(C, S, R, N)>,
}

impl<C, S, R, N> Clone for CqrsDomainDsl<C, S, R, N> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C, S, R, N> Copy for CqrsDomainDsl<C, S, R, N> {}

impl<C, S, R, N> Default for CqrsDomainDsl<C, S, R, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C, S, R, N> CqrsDomainDsl<C, S, R, N> {
    /// Creates the DSL.
    pub const fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl<C, S, R, N> std::fmt::Debug for CqrsDomainDsl<C, S, R, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CqrsDomainDsl")
    }
}

/// The program type produced by a [`CqrsDomainDsl`].
pub type CqrsApp<C, S, R, N, T> = Stomaton<CommandMessage<C>, S, R, N, T>;

impl<C, S, R, N> CqrsDomainDsl<C, S, R, N>
where
    C: Send + 'static,
    S: Send + 'static,
    R: Send + 'static,
    N: Send + 'static,
{
    /// A program that outputs a pure value.
    pub fn pure<T: Clone + Send + Sync + 'static>(self, t: T) -> CqrsApp<C, S, R, N, T> {
        Stomaton::pure(t)
    }

    /// A program with a trivial output.
    pub fn unit(self) -> CqrsApp<C, S, R, N, ()> {
        Stomaton::unit()
    }

    /// A program that evaluates an effect.
    pub fn eval<T, F, Fut>(self, f: F) -> CqrsApp<C, S, R, N, T>
    where
        T: Send + 'static,
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = T> + Send + 'static,
    {
        Stomaton::eval(f)
    }

    /// A program that sets the state.
    pub fn set(self, state: S) -> CqrsApp<C, S, R, N, ()>
    where
        S: Clone + Sync,
    {
        Stomaton::set(state)
    }

    /// A program that modifies the state and outputs the new state.
    pub fn modify<F>(self, f: F) -> CqrsApp<C, S, R, N, S>
    where
        S: Clone,
        F: Fn(S) -> S + Send + Sync + 'static,
    {
        Stomaton::modify(f)
    }

    /// A program that decides on a new state and outputs it.
    pub fn decide_s<F>(self, f: F) -> CqrsApp<C, S, R, N, S>
    where
        S: Clone,
        F: Fn(S) -> Result<S, NonEmpty<R>> + Send + Sync + 'static,
    {
        Stomaton::decide_s(f)
    }

    /// Alias of [`CqrsDomainDsl::decide_s`].
    pub fn modify_s<F>(self, f: F) -> CqrsApp<C, S, R, N, S>
    where
        S: Clone,
        F: Fn(S) -> Result<S, NonEmpty<R>> + Send + Sync + 'static,
    {
        Stomaton::modify_s(f)
    }

    /// A program that outputs a value or rejects.
    pub fn decide<T>(self, result: Result<T, NonEmpty<R>>) -> CqrsApp<C, S, R, N, T>
    where
        T: Send + 'static,
        Result<T, NonEmpty<R>>: Clone + Sync,
    {
        Stomaton::decide(result)
    }

    /// A program that rejects.
    pub fn reject<T>(self, reasons: impl Into<NonEmpty<R>>) -> CqrsApp<C, S, R, N, T>
    where
        T: Send + 'static,
        R: Clone + Sync,
    {
        Stomaton::reject(reasons)
    }

    /// A program from a validation.
    pub fn validate<T>(self, validation: Result<T, NonEmpty<R>>) -> CqrsApp<C, S, R, N, T>
    where
        T: Send + 'static,
        Result<T, NonEmpty<R>>: Clone + Sync,
    {
        Stomaton::validate(validation)
    }

    /// A program from an optional value, rejecting when absent.
    pub fn from_option<T>(
        self,
        opt: Option<T>,
        or_else: impl Into<NonEmpty<R>>,
    ) -> CqrsApp<C, S, R, N, T>
    where
        T: Send + 'static,
        Result<T, NonEmpty<R>>: Clone + Sync,
    {
        Stomaton::from_option(opt, or_else)
    }

    /// A program from a result with a single rejection.
    pub fn from_result<T>(self, result: Result<T, R>) -> CqrsApp<C, S, R, N, T>
    where
        T: Send + 'static,
        Result<T, NonEmpty<R>>: Clone + Sync,
    {
        Stomaton::from_result(result)
    }

    /// A program from a result with several rejections.
    pub fn from_result_nec<T>(self, result: Result<T, NonEmpty<R>>) -> CqrsApp<C, S, R, N, T>
    where
        T: Send + 'static,
        Result<T, NonEmpty<R>>: Clone + Sync,
    {
        Stomaton::from_result_nec(result)
    }

    /// A program that publishes notifications.
    pub fn publish<I>(self, ns: I) -> CqrsApp<C, S, R, N, ()>
    where
        I: IntoIterator<Item = N> + Clone + Send + Sync + 'static,
    {
        Stomaton::publish_only(ns)
    }

    /// A program that outputs the current state.
    pub fn state(self) -> CqrsApp<C, S, R, N, S>
    where
        S: Clone,
    {
        Stomaton::state()
    }

    /// A program that outputs the command message.
    pub fn context(self) -> CqrsApp<C, S, R, N, CommandMessage<C>> {
        Stomaton::context()
    }

    /// A program that outputs the aggregate identifier (the command
    /// address).
    pub fn aggregate_id(self) -> CqrsApp<C, S, R, N, String> {
        Stomaton::map_input(|cmd: CommandMessage<C>| cmd.address)
    }

    /// A program that outputs the command metadata.
    pub fn metadata(self) -> CqrsApp<C, S, R, N, MessageMetadata> {
        Stomaton::map_input(|cmd: CommandMessage<C>| cmd.metadata)
    }

    /// A program that outputs the command message identifier.
    pub fn message_id(self) -> CqrsApp<C, S, R, N, String> {
        Stomaton::map_input(|cmd: CommandMessage<C>| cmd.id)
    }

    /// A program that outputs the command payload.
    pub fn command(self) -> CqrsApp<C, S, R, N, C> {
        Stomaton::map_input(|cmd: CommandMessage<C>| cmd.payload)
    }

    /// Routes the command payload to a program.
    pub fn router<T, F>(self, f: F) -> CqrsApp<C, S, R, N, T>
    where
        C: Clone,
        T: Send + 'static,
        F: Fn(C) -> CqrsApp<C, S, R, N, T> + Send + Sync + 'static,
    {
        self.command().and_then(f)
    }
}
