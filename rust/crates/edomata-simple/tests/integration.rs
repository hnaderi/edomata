//! Port of `JavaApiTest.java` (run by `JavaApiIntegrationSuite.scala`): the
//! whole facade used from plain application code, without any core type.

use chrono::Utc;
use edomata_simple::*;

#[test]
fn either_right_left_fold() {
    let either: Result<i32, String> = Ok(42);
    assert!(either.is_ok());
    assert_eq!(either.clone().unwrap(), 42);
    let either: Result<i32, String> = Err("error".to_string());
    assert!(either.is_err());
    assert_eq!(either.clone().unwrap_err(), "error");
    let right: Result<i32, String> = Ok(10);
    let result = right.map_or_else(|l| format!("left:{l}"), |r| format!("right:{r}"));
    assert_eq!(result, "right:10");
}

#[test]
fn decision_constructors() {
    let dec =
        SimpleDecision::<String, String, ()>::accept(["event1".to_string(), "event2".to_string()]);
    assert!(dec.is_accepted() && !dec.is_rejected() && !dec.is_indecisive());
    let dec: SimpleDecision<String, String, ()> = SimpleDecision::reject(["reason1".to_string()]);
    assert!(dec.is_rejected());
    let dec: SimpleDecision<String, String, String> = SimpleDecision::pure("hello".to_string());
    assert!(dec.is_indecisive());
    let dec: SimpleDecision<String, String, i32> = SimpleDecision::pure(5);
    assert!(dec.map(|x| format!("v={x}")).is_indecisive());
    let dec: SimpleDecision<String, String, i32> =
        SimpleDecision::accept_return(42, ["e1".to_string()]);
    assert!(dec.is_accepted());
}

#[test]
fn command_message() {
    let now = Utc::now();
    let cmd = CommandMessage::new("cmd-1", now, "agg-1", "do-something");
    assert_eq!(cmd.id, "cmd-1");
    assert_eq!(cmd.address, "agg-1");
    assert_eq!(cmd.payload, "do-something");
    assert_eq!(cmd.time, now);
}

#[test]
fn app_result_constructors() {
    let result: AppResult<String, String, String> = AppResult::accept(["event1".to_string()]);
    assert!(result.decision.is_accepted());
    assert!(result.notifications.is_empty());
    let result: AppResult<String, String, String> = AppResult::reject(["bad-input".to_string()]);
    assert!(result.decision.is_rejected());
    let result: AppResult<String, String, String> =
        AppResult::publish(["notif1".to_string(), "notif2".to_string()]);
    assert!(result.decision.is_indecisive());
    assert_eq!(result.notifications.len(), 2);
}

#[test]
fn command_handler_from_a_closure() {
    let handler: CommandHandler<String, i32, String, String, String> = CommandHandler::new(|ctx| {
        let _state: i32 = ctx.state;
        if ctx.command == "increment" {
            AppResult::accept(["incremented".to_string()])
        } else {
            AppResult::reject([format!("unknown command: {}", ctx.command)])
        }
    });
    let ctx = |command: &str| Context {
        command: command.to_string(),
        message: CommandMessage::new("c1", Utc::now(), "a1", command.to_string()),
        state: 0,
    };
    let out = futures::executor::block_on(handler.call(ctx("increment")));
    assert_eq!(out.decision.events(), &["incremented".to_string()]);
    let out = futures::executor::block_on(handler.call(ctx("other")));
    assert_eq!(
        out.decision.reasons(),
        &["unknown command: other".to_string()]
    );
    assert_eq!(ctx("x").address(), "a1");
    assert_eq!(ctx("x").message_id(), "c1");

    let async_handler: CommandHandler<String, i32, String, String, String> =
        CommandHandler::new_async(|ctx| async move { AppResult::publish([ctx.command]) });
    let out = futures::executor::block_on(async_handler.call(ctx("ping")));
    assert_eq!(out.notifications, vec!["ping".to_string()]);
}

#[test]
fn codec_from_closures() {
    let codec = ClosureCodec::<i32>::new(
        |i| i.to_string(),
        |s| s.parse::<i32>().map_err(|e| e.to_string()),
    );
    assert_eq!(codec.encode(&42), "42");
    assert_eq!(codec.decode("99"), Ok(99));
}

#[test]
fn pg_schema_eventsourcing_and_cqrs() {
    let ddl = SimplePGSchema::eventsourcing("myapp").unwrap();
    assert!(!ddl.is_empty());
    let all = ddl.join("\n");
    for table in [
        "myapp_journal",
        "myapp_outbox",
        "myapp_commands",
        "myapp_snapshots",
    ] {
        assert!(all.contains(table), "Expected {table}");
    }
    let all = SimplePGSchema::cqrs("myapp").unwrap().join("\n");
    assert!(all.contains("myapp_states"));
}

#[test]
fn runtime_create_and_close() {
    let runtime = SimpleRuntime::create().unwrap();
    assert!(runtime.owns_runtime());
    assert_eq!(runtime.block_on(async { 1 + 1 }), 2);
    let borrowed = SimpleRuntime::from_handle(runtime.handle().clone());
    assert!(!borrowed.owns_runtime());
    borrowed.close();
    runtime.close();
}
