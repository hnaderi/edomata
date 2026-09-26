//! # Edomata Simple
//!
//! A simplified, closure-based facade over Edomata for event-sourced
//! services on PostgreSQL. It is the Rust counterpart of the Scala
//! `java-api` module: the same capabilities, without touching the generic
//! core types (`Edomaton`, `ResponseD`, `NonEmpty`, ...).
//!
//! | Java API | Here |
//! |----------|------|
//! | `JDomainModel` | [`SimpleDomainModel`] (or [`ClosureModel::new`]) |
//! | `JDecision` | [`SimpleDecision`] |
//! | `JAppResult` | [`AppResult`] |
//! | `JCommandHandler`, `JRequestContext` | [`CommandHandler`], [`Context`] |
//! | `JCodec` | [`SimpleCodec`], [`ClosureCodec::new`] (or [`serde_codec`] for serde types) |
//! | `JCommandMessage` | [`CommandMessage`] |
//! | `JEventMessage`, `JOutboxItem` | [`EventMessage`], [`OutboxItem`] |
//! | `JBackendBuilder`, `JBackend` | [`SimpleBackend::builder`], [`SimpleBackend`] |
//! | `JJournalReader`, `JOutboxReader` | [`SimpleJournal`], [`SimpleOutbox`] |
//! | `EdomataRuntime` | [`SimpleRuntime`] (+ [`BlockingBackend`]) |
//! | `JPGSchema` | [`SimplePGSchema`] |
//! | `JEither` | `Result` |
//!
//! ```no_run
//! use edomata_simple::*;
//!
//! # async fn demo() -> Result<(), SimpleError> {
//! let model = <ClosureModel<i64, i64, String>>::new(0, |event: &i64, balance| {
//!     let next = balance + event;
//!     if next < 0 { Err(vec!["insufficient balance".to_string()]) } else { Ok(next) }
//! });
//!
//! let backend = SimpleBackend::builder(model)
//!     .namespace("accounts")
//!     .database_url("postgres://postgres:postgres@localhost:5432/postgres")
//!     .serde_codecs()
//!     .build()
//!     .await?;
//!
//! let handler: CommandHandler<i64, i64, i64, String, String> = CommandHandler::new(|ctx| {
//!     if ctx.command == 0 { AppResult::reject(["zero".to_string()]) } else { AppResult::accept([ctx.command]) }
//! });
//!
//! let cmd = CommandMessage::new("cmd-1", chrono::Utc::now(), "acc-1", 100);
//! match backend.handle(&handler, cmd).await? {
//!     Ok(()) => println!("accepted"),
//!     Err(reasons) => println!("rejected: {reasons:?}"),
//! }
//! # Ok(()) }
//! ```

#![forbid(unsafe_code)]

mod app_result;
mod backend;
mod codec;
mod decision;
mod error;
mod handler;
mod model;
mod runtime;
mod schema;

pub use app_result::AppResult;
pub use backend::{
    BuiltBackend, BuiltBlockingBackend, HandleResult, SimpleBackend, SimpleBackendBuilder,
    SimpleJournal, SimpleOutbox, SimpleService,
};
pub use codec::{ClosureCodec, CodecAdapter, SimpleCodec, serde_codec};
pub use decision::{EmptyRejection, SimpleDecision};
pub use error::SimpleError;
pub use handler::{CommandHandler, Context};
pub use model::{ClosureModel, ModelAdapter, SimpleDomainModel};
pub use runtime::{BlockingBackend, SimpleRuntime};
pub use schema::SimplePGSchema;

// Re-exports so that applications need no other Edomata crate.
pub use edomata_backend::{BackendError, EventMessage, EventMetadata, OutboxItem};
pub use edomata_core::{CommandMessage, Decision, MessageMetadata};
pub use edomata_sqlx::{PGNamespace, PGNaming, PgPool, SqlxCodec};
