//! Samples of the "Simple API" chapter.

use edomata_simple::*;

// ANCHOR: model
/// A counter: the state is an `i32`, events and rejections are strings.
pub fn counter_model() -> ClosureModel<i32, String, String> {
    ClosureModel::new(0, |event: &String, state: i32| match event.as_str() {
        "increment" => Ok(state + 1),
        "decrement" if state > 0 => Ok(state - 1),
        "decrement" => Err(vec!["cannot go below zero".to_string()]),
        other => Err(vec![format!("unknown event: {other}")]),
    })
}
// ANCHOR_END: model

// ANCHOR: handler
/// Commands are plain strings here; the handler returns an `AppResult`.
pub fn handler() -> CommandHandler<String, i32, String, String, String> {
    CommandHandler::new(|ctx: Context<String, i32>| match ctx.command.as_str() {
        "inc" => AppResult::accept(["increment".to_string()]),
        "dec" => AppResult::accept(["decrement".to_string()]),
        "ping" => AppResult::publish([format!("{} is at {}", ctx.address(), ctx.state)]),
        other => AppResult::reject([format!("unknown: {other}")]),
    })
}
// ANCHOR_END: handler

// ANCHOR: backend
pub async fn run(database_url: &str) -> Result<(), SimpleError> {
    let backend = SimpleBackend::builder(counter_model())
        .namespace("counter") // prefixed naming: counter_journal, ...
        .database_url(database_url)
        .serde_codecs() // events and notifications as jsonb through serde
        .max_retry(5)
        .in_mem_snapshot_size(1000)
        .build()
        .await?;

    let handler = handler();
    let cmd = CommandMessage::new("cmd-1", chrono::Utc::now(), "counter-1", "inc".to_string());
    match backend.handle(&handler, cmd).await? {
        Ok(()) => println!("accepted"),
        Err(reasons) => println!("rejected: {reasons:?}"),
    }

    // Journal and outbox as vectors.
    let events = backend.journal().read_stream("counter-1").await?;
    let pending = backend.outbox().read().await?;
    println!(
        "{} events, {} pending notifications",
        events.len(),
        pending.len()
    );
    backend.close().await
}
// ANCHOR_END: backend

// ANCHOR: blocking
/// For code outside an async runtime.
pub fn run_blocking(database_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let runtime = SimpleRuntime::create()?; // owns a Tokio runtime
    let backend = SimpleBackend::builder(counter_model())
        .namespace("counter")
        .database_url(database_url)
        .serde_codecs()
        .build_blocking(runtime)?;
    let cmd = CommandMessage::new("cmd-2", chrono::Utc::now(), "counter-1", "dec".to_string());
    println!("{:?}", backend.handle(&handler(), cmd)?);
    backend.close()?;
    Ok(())
}
// ANCHOR_END: blocking

// ANCHOR: ddl
pub fn ddl() -> Result<Vec<String>, SimpleError> {
    // Prefixed naming (`myapp_journal`, ...); `*_with_schema` for schema naming.
    SimplePGSchema::eventsourcing("myapp")
}
// ANCHOR_END: ddl

#[cfg(test)]
mod tests {
    use super::*;
    use edomata_core::DomainModel as _;

    #[test]
    fn model_and_handler() {
        // ANCHOR: simple_tests
        let model = counter_model();
        assert_eq!(model.transition(&"increment".to_string(), 1), Ok(2));
        assert_eq!(
            model.transition(&"decrement".to_string(), 0),
            Err(vec!["cannot go below zero".to_string()])
        );
        // Adapted to the core model: rejections become a NonEmpty.
        assert_eq!(
            model
                .clone()
                .into_model()
                .transition(&"increment".to_string(), 0),
            Ok(1)
        );

        let out = futures::executor::block_on(handler().call(Context {
            command: "inc".to_string(),
            message: CommandMessage::new("c", chrono::Utc::now(), "counter-1", "inc".to_string()),
            state: 0,
        }));
        assert_eq!(out.decision.events(), &["increment".to_string()]);

        let decision: SimpleDecision<String, String, ()> =
            SimpleDecision::reject(["nope".to_string()]);
        assert_eq!(decision.to_result(), Err(vec!["nope".to_string()]));
        // ANCHOR_END: simple_tests
        assert!(ddl().unwrap().iter().any(|s| s.contains("myapp_journal")));
    }
}
