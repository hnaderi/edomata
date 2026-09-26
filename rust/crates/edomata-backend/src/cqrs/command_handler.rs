//! Turning a [`Stomaton`] into a command-handling service.

use std::sync::Arc;

use edomata_core::{CommandMessage, NonEmpty, Stomaton};

use super::{AggregateState, CommandState, Repository};
use crate::header::Header;
use crate::{CommandRef, CommandResult, DomainService, Payload, RetryConfig, retry_with};

/// Compiles CQRS programs into services backed by a [`Repository`].
///
/// Handling a command loads the aggregate, runs the program with the
/// command and the current state, and:
///
/// - saves the new state and publishes the notifications when the program
///   succeeded (even when the state did not change);
/// - publishes the notifications and returns the reasons when it rejected;
/// - does nothing for a redundant command.
///
/// With a [`RetryConfig`], version conflicts are retried.
pub struct CommandHandler<S, N> {
    repository: Arc<dyn Repository<S, N>>,
    retry: Option<RetryConfig>,
}

impl<S, N> Clone for CommandHandler<S, N> {
    fn clone(&self) -> Self {
        Self {
            repository: Arc::clone(&self.repository),
            retry: self.retry,
        }
    }
}

impl<S: Payload, N: Payload> CommandHandler<S, N> {
    /// A handler without retries.
    pub fn new(repository: Arc<dyn Repository<S, N>>) -> Self {
        Self {
            repository,
            retry: None,
        }
    }

    /// A handler that retries on version conflicts.
    pub fn with_retry(repository: Arc<dyn Repository<S, N>>, retry: RetryConfig) -> Self {
        Self {
            repository,
            retry: Some(retry),
        }
    }

    /// Compiles `app` into a service.
    pub fn compile<C, R>(
        &self,
        app: Stomaton<CommandMessage<C>, S, R, N, ()>,
    ) -> DomainService<C, R>
    where
        C: Payload,
        R: Payload,
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

    async fn handle_once<C: Payload, R: Payload>(
        &self,
        app: &Stomaton<CommandMessage<C>, S, R, N, ()>,
        cmd: CommandMessage<C>,
    ) -> CommandResult<R> {
        match self.repository.load(CommandRef::from(&cmd)).await? {
            CommandState::Redundant => Ok(Ok(())),
            CommandState::Aggregate(AggregateState { state, version }) => {
                let header = Header::of(&cmd);
                let out = app.run(cmd, state).await;
                match out.result {
                    Ok((new_state, ())) => {
                        self.repository
                            .save(header.as_ref(), version, new_state, out.notifications)
                            .await?;
                        Ok(Ok(()))
                    }
                    Err(reasons) => {
                        if let Some(ns) = NonEmpty::from_vec(out.notifications) {
                            self.repository.notify(header.as_ref(), ns).await?;
                        }
                        Ok(Err(reasons))
                    }
                }
            }
        }
    }
}
