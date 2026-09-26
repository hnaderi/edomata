//! The loop shared by the relays: pass, then wait for a wake-up, the polling
//! fallback or cancellation; publishing with exponential backoff.

use std::future::Future;
use std::sync::Arc;

use edomata_core::NonEmpty;
use futures::StreamExt;
use futures::stream::{BoxStream, SelectAll};
use tokio_util::sync::CancellationToken;

use crate::{BrokerMessage, PublishError, Publisher, RelayConfig, RelayError, RelayMetrics};

/// Runs `pass` once, then again after every wake-up or after
/// `poll_interval`, until `cancel` fires.
pub(crate) async fn run_loop<F, Fut>(
    config: &RelayConfig,
    wakeups: Vec<BoxStream<'static, ()>>,
    cancel: CancellationToken,
    mut pass: F,
) -> Result<(), RelayError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<usize, RelayError>>,
{
    let mut wake: SelectAll<BoxStream<'static, ()>> = futures::stream::select_all(wakeups);
    loop {
        if cancel.is_cancelled() {
            return Ok(());
        }
        pass().await?;
        tokio::select! {
            biased;
            _ = cancel.cancelled() => return Ok(()),
            _ = tokio::time::sleep(config.poll_interval) => {}
            item = wake.next(), if !wake.is_empty() => {
                if item.is_none() {
                    // Every wake-up stream ended: keep polling.
                    tracing::debug!(source = %config.source, "wake-up streams ended, polling only");
                }
            }
        }
    }
}

/// Publishes `batch`, retrying transient failures with the configured
/// backoff. Permanent failures and an exhausted budget stop the relay.
pub(crate) async fn publish_with_retry(
    publisher: &Arc<dyn Publisher>,
    batch: &NonEmpty<BrokerMessage>,
    config: &RelayConfig,
    metrics: &RelayMetrics,
    cancel: &CancellationToken,
) -> Result<(), RelayError> {
    let mut retry = 0u32;
    loop {
        match publisher.publish(batch).await {
            Ok(()) => return Ok(()),
            Err(PublishError::Transient(e)) => {
                retry += 1;
                if config.retry.exhausted(retry) {
                    metrics.add_failed();
                    tracing::error!(source = %config.source, error = %e, retries = retry - 1, "publish retries exhausted");
                    return Err(RelayError::Publish(PublishError::Transient(e)));
                }
                metrics.add_retried();
                let delay = config.retry.delay_for(retry);
                tracing::warn!(source = %config.source, error = %e, retry, delay_ms = delay.as_millis() as u64, "transient publish failure, retrying");
                tokio::select! {
                    _ = cancel.cancelled() => return Err(RelayError::Cancelled),
                    _ = tokio::time::sleep(delay) => {}
                }
            }
            Err(e @ PublishError::Permanent(_)) => {
                metrics.add_failed();
                tracing::error!(source = %config.source, error = %e, "permanent publish failure");
                return Err(RelayError::Publish(e));
            }
        }
    }
}
