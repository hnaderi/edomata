//! The serde_json codec.

use std::marker::PhantomData;

use edomata_backend::{Codec, CodecError, PayloadFormat};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// A [`Codec`] for any `T: Serialize + DeserializeOwned`, backed by
/// `serde_json`.
///
/// The format only selects the PostgreSQL column type the bytes are stored
/// in; the bytes are always compact JSON (`serde_json::to_writer`), so a
/// `bytea` payload written by this codec is readable as JSON too.
///
/// `SerdeCodec` is a small `Copy` value (just the format): it is `Default` (`jsonb`) and
/// can be passed to `BackendBuilder::build`.
pub struct SerdeCodec<T> {
    format: PayloadFormat,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for SerdeCodec<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for SerdeCodec<T> {}

impl<T> std::fmt::Debug for SerdeCodec<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SerdeCodec")
            .field("format", &self.format)
            .finish()
    }
}

impl<T> Default for SerdeCodec<T> {
    /// The `jsonb` codec.
    fn default() -> Self {
        Self::jsonb()
    }
}

impl<T> SerdeCodec<T> {
    /// A codec storing JSON in a `jsonb` column (the default).
    pub const fn jsonb() -> Self {
        Self::with_format(PayloadFormat::Jsonb)
    }

    /// A codec storing JSON in a `json` column.
    pub const fn json() -> Self {
        Self::with_format(PayloadFormat::Json)
    }

    /// A codec storing JSON bytes in a `bytea` column.
    pub const fn bytea() -> Self {
        Self::with_format(PayloadFormat::Bytea)
    }

    /// A codec for the given column type.
    pub const fn with_format(format: PayloadFormat) -> Self {
        Self {
            format,
            _marker: PhantomData,
        }
    }
}

impl<T> Codec<T> for SerdeCodec<T>
where
    T: Serialize + DeserializeOwned + Send + Sync,
{
    fn format(&self) -> PayloadFormat {
        self.format
    }

    fn encode(&self, value: &T) -> Result<Vec<u8>, CodecError> {
        let mut out = Vec::new();
        serde_json::to_writer(&mut out, value).map_err(|e| CodecError::Encode(e.to_string()))?;
        Ok(out)
    }

    fn decode(&self, bytes: &[u8]) -> Result<T, CodecError> {
        serde_json::from_slice(bytes).map_err(|e| CodecError::Decode(e.to_string()))
    }
}
