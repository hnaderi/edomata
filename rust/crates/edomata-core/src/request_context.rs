//! Command messages and the request context handed to domain programs.

use chrono::{DateTime, Utc};

/// The input of an [`Edomaton`](crate::Edomaton): the command being handled
/// together with the current state of the aggregate.
///
/// Backends build it from the incoming [`CommandMessage`] and the state read
/// from storage; in tests, build it with [`CommandMessage::build_context`]
/// or [`RequestContext::new`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestContext<C, S> {
    /// The command message.
    pub command: CommandMessage<C>,
    /// Current state of the aggregate.
    pub state: S,
}

impl<C, S> RequestContext<C, S> {
    /// Builds a request context.
    pub fn new(command: CommandMessage<C>, state: S) -> Self {
        Self { command, state }
    }
}

/// A command message sent to an aggregate.
///
/// Backends use [`CommandMessage::id`] to make command handling idempotent
/// (a command id is processed at most once) and
/// [`CommandMessage::address`] as the aggregate's stream id.
///
/// ```
/// use edomata_core::{CommandMessage, MessageMetadata};
///
/// let cmd = CommandMessage::new("cmd-1", chrono::DateTime::UNIX_EPOCH, "account-42", "deposit");
/// // A root message is its own correlation and causation.
/// assert_eq!(cmd.metadata, MessageMetadata::root("cmd-1"));
///
/// // A message caused by `cmd` keeps the correlation and points back to it.
/// let next = cmd.derive_meta();
/// assert_eq!(next.correlation.as_deref(), Some("cmd-1"));
/// assert_eq!(next.causation.as_deref(), Some("cmd-1"));
///
/// let ctx = cmd.build_context(100_i64);
/// assert_eq!(ctx.command.address, "account-42");
/// assert_eq!(ctx.state, 100);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CommandMessage<C> {
    /// Unique identifier of this message; used for idempotency.
    pub id: String,
    /// When the command was issued.
    pub time: DateTime<Utc>,
    /// Address of the target aggregate (its stream identifier).
    pub address: String,
    /// The command payload, your command model.
    pub payload: C,
    /// Correlation and causation metadata.
    pub metadata: MessageMetadata,
}

impl<C> CommandMessage<C> {
    /// Constructs a command message at the root of a chain of messages.
    pub fn new(
        id: impl Into<String>,
        time: DateTime<Utc>,
        address: impl Into<String>,
        payload: C,
    ) -> Self {
        let id = id.into();
        let metadata = MessageMetadata::root(id.clone());
        Self {
            id,
            time,
            address: address.into(),
            payload,
            metadata,
        }
    }

    /// Constructs a command message with explicit metadata.
    pub fn with_metadata(
        id: impl Into<String>,
        time: DateTime<Utc>,
        address: impl Into<String>,
        payload: C,
        metadata: MessageMetadata,
    ) -> Self {
        Self {
            id: id.into(),
            time,
            address: address.into(),
            payload,
            metadata,
        }
    }

    /// Builds a request context from this message.
    pub fn build_context<S>(self, state: S) -> RequestContext<C, S> {
        RequestContext {
            command: self,
            state,
        }
    }

    /// Derives the metadata of the next message in the chain that is caused
    /// by this one.
    pub fn derive_meta(&self) -> MessageMetadata {
        MessageMetadata {
            correlation: self.metadata.correlation.clone(),
            causation: Some(self.id.clone()),
        }
    }

    /// Changes the payload.
    pub fn map<D, F: FnOnce(C) -> D>(self, f: F) -> CommandMessage<D> {
        CommandMessage {
            id: self.id,
            time: self.time,
            address: self.address,
            payload: f(self.payload),
            metadata: self.metadata,
        }
    }
}

/// Correlation and causation identifiers of a message.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MessageMetadata {
    /// Identifier of the whole chain of messages.
    pub correlation: Option<String>,
    /// Identifier of the message that directly caused this one.
    pub causation: Option<String>,
}

impl MessageMetadata {
    /// Constructs the metadata of a root message: correlation and causation
    /// are both the message id.
    pub fn root(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            correlation: Some(id.clone()),
            causation: Some(id),
        }
    }

    /// Constructs metadata with explicit identifiers.
    pub fn new(correlation: impl Into<String>, causation: impl Into<String>) -> Self {
        Self {
            correlation: Some(correlation.into()),
            causation: Some(causation.into()),
        }
    }

    /// Metadata without any identifier.
    pub fn empty() -> Self {
        Self::default()
    }
}
