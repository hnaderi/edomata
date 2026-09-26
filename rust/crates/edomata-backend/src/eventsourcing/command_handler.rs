//! Turning an [`Edomaton`] into a command-handling service.

use std::sync::Arc;

use edomata_core::{CommandMessage, Edomaton, EdomatonResult, NonEmpty, RequestContext};

use super::{AggregateState, CommandState, Repository, ValidState};
use crate::header::Header;
use crate::{
    CommandRef, CommandResult, DomainService, Payload, RetryConfig, SharedModel, retry_with,
};

/// Compiles domain programs into services backed by a [`Repository`].
///
/// Handling a command loads the aggregate, runs the program, and:
///
/// - appends the accepted events (and notifications) when the program
///   accepted;
/// - publishes the notifications when it was indecisive or rejected;
/// - returns the rejection reasons when it rejected, or when the aggregate
///   is conflicted;
/// - does nothing for a redundant command.
///
/// With a [`RetryConfig`], [`BackendError::VersionConflict`](crate::BackendError::VersionConflict) is retried.
pub struct CommandHandler<S, E, R, N> {
    repository: Arc<dyn Repository<S, E, R, N>>,
    model: SharedModel<S, E, R>,
    retry: Option<RetryConfig>,
}

impl<S, E, R, N> Clone for CommandHandler<S, E, R, N> {
    fn clone(&self) -> Self {
        Self {
            repository: Arc::clone(&self.repository),
            model: Arc::clone(&self.model),
            retry: self.retry,
        }
    }
}

impl<S: Payload, E: Payload, R: Payload, N: Payload> CommandHandler<S, E, R, N> {
    /// A handler without retries.
    pub fn new(repository: Arc<dyn Repository<S, E, R, N>>, model: SharedModel<S, E, R>) -> Self {
        Self {
            repository,
            model,
            retry: None,
        }
    }

    /// A handler that retries on version conflicts.
    pub fn with_retry(
        repository: Arc<dyn Repository<S, E, R, N>>,
        model: SharedModel<S, E, R>,
        retry: RetryConfig,
    ) -> Self {
        Self {
            repository,
            model,
            retry: Some(retry),
        }
    }

    /// Compiles `app` into a service.
    pub fn compile<C>(
        &self,
        app: Edomaton<RequestContext<C, S>, R, E, N, ()>,
    ) -> DomainService<C, R>
    where
        C: Payload,
    {
        let handler = self.clone();
        Arc::new(move |cmd: CommandMessage<C>| {
            let handler = handler.clone();
            let app = app.clone();
            Box::pin(async move {
                match handler.retry {
                    None => handler.handle_once(&app, cmd).await,
                    Some(config) => {
                        retry_with(config, || handler.handle_once(&app, cmd.clone())).await
                    }
                }
            })
        })
    }

    async fn handle_once<C: Payload>(
        &self,
        app: &Edomaton<RequestContext<C, S>, R, E, N, ()>,
        cmd: CommandMessage<C>,
    ) -> CommandResult<R> {
        match self.repository.load(CommandRef::from(&cmd)).await? {
            CommandState::Redundant => Ok(Ok(())),
            CommandState::Aggregate(AggregateState::Conflicted { errors, .. }) => Ok(Err(errors)),
            CommandState::Aggregate(AggregateState::Valid(ValidState { state, version })) => {
                let header = Header::of(&cmd);
                let ctx = cmd.build_context(state);
                match app.execute(&*self.model, ctx).await {
                    EdomatonResult::Accepted {
                        new_state,
                        events,
                        notifications,
                    } => {
                        self.repository
                            .append(header.as_ref(), version, new_state, events, notifications)
                            .await?;
                        Ok(Ok(()))
                    }
                    EdomatonResult::Indecisive { notifications } => {
                        if let Some(ns) = NonEmpty::from_vec(notifications) {
                            self.repository.notify(header.as_ref(), ns).await?;
                        }
                        Ok(Ok(()))
                    }
                    EdomatonResult::Rejected {
                        notifications,
                        reasons,
                    } => {
                        if let Some(ns) = NonEmpty::from_vec(notifications) {
                            self.repository.notify(header.as_ref(), ns).await?;
                        }
                        Ok(Err(reasons))
                    }
                    EdomatonResult::Conflicted { reasons } => Ok(Err(reasons)),
                }
            }
        }
    }
}
