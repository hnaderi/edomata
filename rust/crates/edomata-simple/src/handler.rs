//! Command handlers, the counterpart of `JCommandHandler` /
//! `JRequestContext`.

use std::future::Future;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use edomata_core::{BoxFuture, CommandMessage, RequestContext};

use crate::AppResult;

/// What a handler sees: the command, its message and the current state.
/// Mirrors Scala's `JRequestContext`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Context<C, S> {
    /// The command payload.
    pub command: C,
    /// The full command message.
    pub message: CommandMessage<C>,
    /// The aggregate's current state.
    pub state: S,
}

impl<C: Clone, S> Context<C, S> {
    /// Builds a context from a core [`RequestContext`].
    pub fn from_request(ctx: RequestContext<C, S>) -> Self {
        Self {
            command: ctx.command.payload.clone(),
            message: ctx.command,
            state: ctx.state,
        }
    }
}

impl<C, S> Context<C, S> {
    /// The aggregate address.
    pub fn address(&self) -> &str {
        &self.message.address
    }

    /// The command message id.
    pub fn message_id(&self) -> &str {
        &self.message.id
    }

    /// When the command was issued.
    pub fn time(&self) -> DateTime<Utc> {
        self.message.time
    }
}

type HandlerFn<C, S, E, R, N> =
    dyn Fn(Context<C, S>) -> BoxFuture<'static, AppResult<R, E, N>> + Send + Sync;

/// A command handler: a function from a [`Context`] to an [`AppResult`].
/// Mirrors Scala's `JCommandHandler`, with a blocking
/// ([`CommandHandler::new`]) and an asynchronous
/// ([`CommandHandler::new_async`]) constructor.
pub struct CommandHandler<C, S, E, R, N> {
    run: Arc<HandlerFn<C, S, E, R, N>>,
}

impl<C, S, E, R, N> Clone for CommandHandler<C, S, E, R, N> {
    fn clone(&self) -> Self {
        Self {
            run: Arc::clone(&self.run),
        }
    }
}

impl<C, S, E, R, N> std::fmt::Debug for CommandHandler<C, S, E, R, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CommandHandler")
    }
}

impl<C, S, E, R, N> CommandHandler<C, S, E, R, N>
where
    C: Send + 'static,
    S: Send + 'static,
    E: Send + 'static,
    R: Send + 'static,
    N: Send + 'static,
{
    /// A handler from a plain function (`JCommandHandler.create`).
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(Context<C, S>) -> AppResult<R, E, N> + Send + Sync + 'static,
    {
        Self {
            run: Arc::new(move |ctx| {
                let out = f(ctx);
                Box::pin(async move { out })
            }),
        }
    }

    /// A handler from an asynchronous function.
    pub fn new_async<F, Fut>(f: F) -> Self
    where
        F: Fn(Context<C, S>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = AppResult<R, E, N>> + Send + 'static,
    {
        Self {
            run: Arc::new(move |ctx| Box::pin(f(ctx))),
        }
    }

    /// Runs the handler.
    pub fn call(&self, ctx: Context<C, S>) -> BoxFuture<'static, AppResult<R, E, N>> {
        (self.run)(ctx)
    }
}
