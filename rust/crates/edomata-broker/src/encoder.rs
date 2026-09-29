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
/// reuses any storage [`Codec`], and [`MessageEncoder::new`] takes any
/// function. The encoder also decides the message's
/// [`content_type`](crate::BrokerMessage::content_type).
///
/// ```
/// use edomata_broker::{APPLICATION_OCTET_STREAM, MessageEncoder};
///
/// #[derive(serde::Serialize)]
/// struct Deposited { amount: i64 }
///
/// let json = MessageEncoder::<Deposited>::serde();
/// assert_eq!(json.content_type(), "application/json");
/// assert_eq!(json.encode(&Deposited { amount: 5 }).unwrap(), br#"{"amount":5}"#);
///
/// // Any function: here a fixed-width binary encoding.
/// let binary = MessageEncoder::<Deposited>::new(APPLICATION_OCTET_STREAM, |d| {
///     Ok(d.amount.to_be_bytes().to_vec())
/// });
/// assert_eq!(binary.encode(&Deposited { amount: 1 }).unwrap(), [0, 0, 0, 0, 0, 0, 0, 1]);
///
/// // Storage codecs are reused as they are.
/// let from_codec = MessageEncoder::from_codec(edomata_serde::SerdeCodec::<i64>::jsonb());
/// assert_eq!(from_codec.content_type(), "application/json");
/// ```
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

/// The JSON content type, used by [`MessageEncoder::serde`] and for `json` /
/// `jsonb` codecs.
pub const APPLICATION_JSON: &str = "application/json";
/// The binary content type, used for `bytea` codecs.
pub const APPLICATION_OCTET_STREAM: &str = "application/octet-stream";

impl<T> MessageEncoder<T> {
    /// An encoder from a content type and a function. The function reports
    /// failures as [`RelayError::Encode`], which stops the relay: a payload
    /// that cannot be encoded is not skipped.
    pub fn new<F>(content_type: impl Into<String>, encode: F) -> Self
    where
        F: Fn(&T) -> Result<Vec<u8>, RelayError> + Send + Sync + 'static,
    {
        Self {
            content_type: content_type.into(),
            encode: Arc::new(encode),
        }
    }

    /// JSON through `serde_json` (`application/json`): the same bytes that
    /// `edomata-serde` writes in `json` / `jsonb` columns.
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
    ///
    /// # Errors
    ///
    /// [`RelayError::Encode`] when the underlying function or codec fails.
    pub fn encode(&self, value: &T) -> Result<Vec<u8>, RelayError> {
        (self.encode)(value)
    }
}
