//! Owned copy of a command header, used by the command handlers while the
//! program consumes the command message.

use chrono::{DateTime, Utc};
use edomata_core::{CommandMessage, MessageMetadata};

use crate::CommandRef;

pub(crate) struct Header {
    id: String,
    time: DateTime<Utc>,
    address: String,
    metadata: MessageMetadata,
}

impl Header {
    pub(crate) fn of<C>(cmd: &CommandMessage<C>) -> Self {
        Self {
            id: cmd.id.clone(),
            time: cmd.time,
            address: cmd.address.clone(),
            metadata: cmd.metadata.clone(),
        }
    }

    pub(crate) fn as_ref(&self) -> CommandRef<'_> {
        CommandRef {
            id: &self.id,
            time: self.time,
            address: &self.address,
            metadata: &self.metadata,
        }
    }
}
