//! Port of `JCommandMessageSuite.scala`. `JCommandMessage` maps to the core
//! `CommandMessage`, re-exported by `edomata_simple`.

use chrono::{DateTime, Utc};
use edomata_simple::CommandMessage;

#[test]
fn new_creates_command_message_with_correct_fields() {
    let time: DateTime<Utc> = "2024-01-01T00:00:00Z".parse().unwrap();
    let cmd = CommandMessage::new("cmd-1", time, "aggregate-1", "payload");
    assert_eq!(cmd.id, "cmd-1");
    assert_eq!(cmd.time, time);
    assert_eq!(cmd.address, "aggregate-1");
    assert_eq!(cmd.payload, "payload");
}

#[test]
fn metadata_is_rooted_at_the_command() {
    let cmd = CommandMessage::new("id", Utc::now(), "addr", 42);
    assert_eq!(cmd.id, "id");
    assert_eq!(cmd.address, "addr");
    assert_eq!(cmd.payload, 42);
    assert_eq!(cmd.metadata.correlation.as_deref(), Some("id"));
    assert_eq!(cmd.metadata.causation.as_deref(), Some("id"));
}

#[test]
fn equality_works_on_all_fields() {
    let time: DateTime<Utc> = "2024-06-15T12:00:00Z".parse().unwrap();
    let cmd1 = CommandMessage::new("id", time, "addr", "p");
    let cmd2 = CommandMessage::new("id", time, "addr", "p");
    assert_eq!(cmd1, cmd2);
    assert_ne!(cmd1, CommandMessage::new("id2", time, "addr", "p"));
}

#[test]
fn debug_includes_all_fields() {
    let cmd = CommandMessage::new("id", DateTime::UNIX_EPOCH, "addr", "data");
    let s = format!("{cmd:?}");
    assert!(s.contains("id"));
    assert!(s.contains("addr"));
    assert!(s.contains("data"));
}
