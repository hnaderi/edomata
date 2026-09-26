//! Payload codecs.
//!
//! A [`Codec`] turns a payload (event, notification, state) into bytes and
//! back, and declares the PostgreSQL column type those bytes are stored in.
//! Storage drivers bridge it to their wire protocol. `edomata-serde`
//! implements it for any `T: Serialize + DeserializeOwned`.

use std::sync::Arc;

/// PostgreSQL column type of a payload.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PayloadFormat {
    /// `json`: JSON text.
    Json,
    /// `jsonb` (the default): binary JSON, queryable with `->`, `@>` and GIN
    /// indexes.
    #[default]
    Jsonb,
    /// `bytea`: opaque bytes.
    Bytea,
}

impl PayloadFormat {
    /// The SQL type name, as used in DDL.
    pub fn sql_type(self) -> &'static str {
        match self {
            PayloadFormat::Json => "json",
            PayloadFormat::Jsonb => "jsonb",
            PayloadFormat::Bytea => "bytea",
        }
    }
}

/// Encoding or decoding failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CodecError {
    /// The value could not be encoded.
    #[error("encoding failed: {0}")]
    Encode(String),
    /// The bytes could not be decoded.
    #[error("decoding failed: {0}")]
    Decode(String),
}

/// Serialises payloads of type `T`.
pub trait Codec<T>: Send + Sync {
    /// Column type the encoded bytes are stored in.
    fn format(&self) -> PayloadFormat;
    /// Encodes a value.
    fn encode(&self, value: &T) -> Result<Vec<u8>, CodecError>;
    /// Decodes a value.
    fn decode(&self, bytes: &[u8]) -> Result<T, CodecError>;
}

impl<T, C: Codec<T> + ?Sized> Codec<T> for Arc<C> {
    fn format(&self) -> PayloadFormat {
        (**self).format()
    }

    fn encode(&self, value: &T) -> Result<Vec<u8>, CodecError> {
        (**self).encode(value)
    }

    fn decode(&self, bytes: &[u8]) -> Result<T, CodecError> {
        (**self).decode(bytes)
    }
}

impl<T, C: Codec<T> + ?Sized> Codec<T> for Box<C> {
    fn format(&self) -> PayloadFormat {
        (**self).format()
    }

    fn encode(&self, value: &T) -> Result<Vec<u8>, CodecError> {
        (**self).encode(value)
    }

    fn decode(&self, bytes: &[u8]) -> Result<T, CodecError> {
        (**self).decode(bytes)
    }
}
