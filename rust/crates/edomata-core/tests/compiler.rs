//! Tests for `DomainCompiler`, `DomainDsl` and `CqrsDomainDsl`.
//!
//! The Scala core module has no dedicated suite for these (they are
//! exercised by the backend suites); these tests pin the `EdomatonResult`
//! semantics described in `docs/principles`.

use chrono::DateTime;
use edomata_core::{
    CommandMessage, CqrsModel, Decision, DomainCompiler, DomainModel, EdomatonResult,
    MessageMetadata, NonEmpty, RequestContext, ResponseE, nonempty,
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Command {
    Add(i32),
    Noop,
    Fail,
    Conflict,
}

struct Counter;

impl DomainModel for Counter {
    type State = i32;
    type Event = i32;
    type Rejection = String;

    fn initial(&self) -> i32 {
        0
    }

    fn transition(&self, event: &i32, state: i32) -> Result<i32, NonEmpty<String>> {
        if *event > 0 {
            Ok(state + event)
        } else {
            Err(NonEmpty::new("negative".to_string()))
        }
    }
}

fn ctx(cmd: Command, state: i32) -> RequestContext<Command, i32> {
    CommandMessage::new("id-1", DateTime::UNIX_EPOCH, "counter-1", cmd).build_context(state)
}

#[test]
fn execute_covers_every_outcome() {
    let dsl = Counter.dsl::<Command, String>();
    let app = dsl.router(move |cmd| match cmd {
        Command::Add(n) => dsl.accept(n).publish(["added".to_string()]),
        Command::Noop => dsl.publish(["noop".to_string()]),
        Command::Fail => dsl
            .reject("failed".to_string())
            .publish(["failed".to_string()]),
        // The program accepts an event the model refuses: a conflict.
        Command::Conflict => dsl.accept(-1),
    });

    let run = |cmd, state| futures::executor::block_on(app.execute(&Counter, ctx(cmd, state)));

    assert_eq!(
        run(Command::Add(5), 10),
        EdomatonResult::Accepted {
            new_state: 15,
            events: nonempty![5],
            notifications: vec!["added".to_string()],
        }
    );
    assert_eq!(
        run(Command::Noop, 10),
        EdomatonResult::Indecisive {
            notifications: vec!["noop".to_string()]
        }
    );
    assert_eq!(
        run(Command::Fail, 10),
        EdomatonResult::Rejected {
            notifications: vec!["failed".to_string()],
            reasons: nonempty!["failed".to_string()],
        }
    );
    assert_eq!(
        run(Command::Conflict, 10),
        EdomatonResult::Conflicted {
            reasons: nonempty!["negative".to_string()],
        }
    );

    // The compiler can also be called directly, with a non-unit output.
    let app2 = dsl.state().map(|s| s * 2);
    assert_eq!(
        futures::executor::block_on(DomainCompiler::execute(
            &Counter,
            &app2,
            ctx(Command::Noop, 3)
        )),
        EdomatonResult::Indecisive {
            notifications: vec![]
        }
    );
}

#[test]
fn domain_dsl_readers() {
    let dsl = Counter.dsl::<Command, String>();
    let context = ctx(Command::Add(1), 42);
    let run = |app: edomata_core::App<Command, i32, i32, String, String, String>| {
        futures::executor::block_on(app.run(context.clone()))
            .result
            .to_option()
            .unwrap()
    };

    assert_eq!(run(dsl.aggregate_id()), "counter-1");
    assert_eq!(run(dsl.message_id()), "id-1");
    assert_eq!(run(dsl.metadata().map(|m| m.correlation.unwrap())), "id-1");
    assert_eq!(run(dsl.state().map(|s| s.to_string())), "42");
    assert_eq!(run(dsl.command().map(|c| format!("{c:?}"))), "Add(1)");
    assert_eq!(run(dsl.command_message().map(|c| c.address)), "counter-1");
    assert_eq!(run(dsl.read().map(|c| c.command.id)), "id-1");
    assert_eq!(run(dsl.pure("p".to_string())), "p");
    assert_eq!(run(dsl.eval(|| async { "e".to_string() })), "e");
    assert_eq!(
        run(dsl.run(|c| async move { c.command.address })),
        "counter-1"
    );
    assert_eq!(
        run(dsl.lift(edomata_core::ResponseD::pure("l".to_string()))),
        "l"
    );
    assert_eq!(run(dsl.decide(Decision::pure("d".to_string()))), "d");
    assert_eq!(
        run(dsl.decide_with(|c| Decision::pure(c.command.id))),
        "id-1"
    );
    assert_eq!(run(dsl.validate(Ok("v".to_string()))), "v");
    assert_eq!(
        run(dsl.from_option(Some("o".to_string()), "none".to_string())),
        "o"
    );
    assert_eq!(run(dsl.from_result(Ok("r".to_string()))), "r");
    assert_eq!(run(dsl.from_result_nec(Ok("n".to_string()))), "n");
    let rejected =
        futures::executor::block_on(dsl.reject::<String>("x".to_string()).run(context.clone()));
    assert!(rejected.result.is_rejected());
    let unit = futures::executor::block_on(dsl.unit().run(context));
    assert_eq!(unit.result, Decision::unit());
}

struct Tally;

impl CqrsModel for Tally {
    type State = i32;
    type Rejection = String;

    fn initial(&self) -> i32 {
        0
    }
}

#[test]
fn cqrs_dsl() {
    let dsl = Tally.dsl::<Command, String>();
    let app = dsl.router(move |cmd| match cmd {
        Command::Add(n) => dsl
            .modify(move |s| s + n)
            .void()
            .publish(["added".to_string()]),
        Command::Noop => dsl.unit(),
        Command::Fail => dsl.reject("failed".to_string()),
        Command::Conflict => dsl
            .modify_s(|_| Err(NonEmpty::new("conflict".to_string())))
            .void(),
    });
    let cmd = |c| CommandMessage::new("id-1", DateTime::UNIX_EPOCH, "tally-1", c);
    let run = |c, s| futures::executor::block_on(app.run(cmd(c), s));

    assert_eq!(
        run(Command::Add(2), 3),
        ResponseE::new(Ok((5, ())), ["added".to_string()])
    );
    assert_eq!(run(Command::Noop, 3), ResponseE::lift(Ok((3, ()))));
    assert_eq!(
        run(Command::Fail, 3),
        ResponseE::reject("failed".to_string())
    );
    assert_eq!(
        run(Command::Conflict, 3),
        ResponseE::reject("conflict".to_string())
    );

    let read = |app: edomata_core::CqrsApp<Command, i32, String, String, String>| {
        futures::executor::block_on(app.run(cmd(Command::Noop), 9))
            .result
            .unwrap()
            .1
    };
    assert_eq!(read(dsl.aggregate_id()), "tally-1");
    assert_eq!(read(dsl.message_id()), "id-1");
    assert_eq!(read(dsl.metadata().map(|m| m.causation.unwrap())), "id-1");
    assert_eq!(read(dsl.state().map(|s| s.to_string())), "9");
    assert_eq!(read(dsl.context().map(|c| c.address)), "tally-1");
    assert_eq!(read(dsl.command().map(|c| format!("{c:?}"))), "Noop");
    assert_eq!(read(dsl.pure("p".to_string())), "p");
    assert_eq!(read(dsl.eval(|| async { "e".to_string() })), "e");
    assert_eq!(read(dsl.decide(Ok("d".to_string()))), "d");
    assert_eq!(read(dsl.validate(Ok("v".to_string()))), "v");
    assert_eq!(
        read(dsl.from_option(Some("o".to_string()), "none".to_string())),
        "o"
    );
    assert_eq!(read(dsl.from_result(Ok("r".to_string()))), "r");
    assert_eq!(read(dsl.from_result_nec(Ok("n".to_string()))), "n");
    assert_eq!(
        read(dsl.decide_s(|s| Ok(s + 1)).map(|s| s.to_string())),
        "10"
    );
    assert_eq!(
        futures::executor::block_on(dsl.set(1).run(cmd(Command::Noop), 9)),
        ResponseE::lift(Ok((1, ())))
    );
    assert_eq!(
        futures::executor::block_on(dsl.publish(["n".to_string()]).run(cmd(Command::Noop), 9)),
        ResponseE::new(Ok((9, ())), ["n".to_string()])
    );
}

#[test]
fn command_message_metadata() {
    let root = CommandMessage::new("a", DateTime::UNIX_EPOCH, "addr", 1);
    assert_eq!(root.metadata, MessageMetadata::root("a"));
    assert_eq!(root.metadata, MessageMetadata::new("a", "a"));
    let derived = root.derive_meta();
    assert_eq!(derived, MessageMetadata::new("a", "a"));

    let child = CommandMessage::with_metadata("b", DateTime::UNIX_EPOCH, "addr", 2, derived);
    assert_eq!(child.derive_meta(), MessageMetadata::new("a", "b"));
    assert_eq!(child.map(|p| p * 10).payload, 20);
    assert_eq!(MessageMetadata::empty(), MessageMetadata::default());
}
