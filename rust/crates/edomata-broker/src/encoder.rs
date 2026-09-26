//! Payload encoding for broker messages.

use std::sync::Arc;

use edomata_backend::{Codec, PayloadFormat};
use serde::Serialize;

use crate::RelayError;

type EncodeFn<T> = dyn Fn(&T) -> Result<Vec<u8>, RelayError> + Send + Sync;

/// Encodes notifications or events into message payloads.
///
/// [`MessageEncoder::serde`] writes JSON with `serde_json`; the JSON is the
/// one `edomata-serde` stores in `jsonb` columns, so brokers and the
/// database share one representation. [`MessageEncoder::from_codec`]
/// reuses any storage [`Codec`].
pub struct MessageEncoder<T> {
    content_type: String,
    encode: Arc<EncodeFn<T>>,
}

impl<T> Clone for MessageEncoder<T> {
    fn clone(&self) -> Self {
        Self {
            content_type: self.content_type.clone(),
            encode: Arc::clone(&self.encode),
        }
    }
}

impl<T> std::fmt::Debug for MessageEncoder<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageEncoder")
            .field("content_type", &self.content_type)
            .finish()
    }
}

/// The JSON content type.
pub const APPLICATION_JSON: &str = "application/json";
/// The binary content type.
pub const APPLICATION_OCTET_STREAM: &str = "application/octet-stream";

impl<T> MessageEncoder<T> {
    /// An encoder from a function and a content type.
    pub fn new<F>(content_type: impl Into<String>, encode: F) -> Self
    where
        F: Fn(&T) -> Result<Vec<u8>, RelayError> + Send + Sync + 'static,
    {
        Self {
            content_type: content_type.into(),
            encode: Arc::new(encode),
        }
    }

    /// JSON through `serde_json` (`application/json`).
    pub fn serde() -> Self
    where
        T: Serialize,
    {
        Self::new(APPLICATION_JSON, |value| {
            serde_json::to_vec(value).map_err(|e| RelayError::Encode(e.to_string()))
        })
    }

    /// The bytes of a storage codec: `application/json` for the `json` and
    /// `jsonb` formats, `application/octet-stream` for `bytea`.
    pub fn from_codec(codec: impl Codec<T> + 'static) -> Self {
        let content_type = match codec.format() {
            PayloadFormat::Json | PayloadFormat::Jsonb => APPLICATION_JSON,
            PayloadFormat::Bytea => APPLICATION_OCTET_STREAM,
        };
        Self::new(content_type, move |value| {
            codec
                .encode(value)
                .map_err(|e| RelayError::Encode(e.to_string()))
        })
    }

    /// The content type of the payloads.
    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    /// Encodes a value.
    pub fn encode(&self, value: &T) -> Result<Vec<u8>, RelayError> {
        (self.encode)(value)
    }
}
