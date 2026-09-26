//! String-based codecs, the counterpart of `JCodec`.

use std::sync::Arc;

use edomata_backend::{Codec, CodecError, PayloadFormat};
use edomata_serde::SerdeCodec;
use edomata_sqlx::SqlxCodec;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// A JSON codec working on strings, stored in `jsonb` columns. Mirrors
/// Scala's `JCodec`: implement it or build one from closures with
/// [`ClosureCodec::new`] (`JCodec.of`). Serde types need no hand-written
/// codec: use [`serde_codec`] (or the builder's `serde_codecs`).
pub trait SimpleCodec<T>: Send + Sync + 'static {
    /// Encodes a value as JSON text.
    fn encode(&self, value: &T) -> String;

    /// Decodes JSON text; the error message is reported as a decoding
    /// failure (Scala catches the exception and returns `Left(message)`).
    fn decode(&self, json: &str) -> Result<T, String>;

    /// Adapts this codec to the storage codec used by the backend
    /// (`JCodec.toBackendCodec`): a `jsonb` codec whose decoding failures
    /// carry the message returned by [`SimpleCodec::decode`].
    fn into_codec(self) -> SqlxCodec<T>
    where
        Self: Sized,
        T: 'static,
    {
        SqlxCodec::new(CodecAdapter(self))
    }
}

/// The `jsonb` storage codec of a serde type: the usual choice in Rust,
/// where no hand-written codec is needed.
pub fn serde_codec<T>() -> SqlxCodec<T>
where
    T: Serialize + DeserializeOwned + Send + Sync + 'static,
{
    SqlxCodec::new(SerdeCodec::<T>::jsonb())
}

type Encoder<T> = dyn Fn(&T) -> String + Send + Sync;
type Decoder<T> = dyn Fn(&str) -> Result<T, String> + Send + Sync;

/// A [`SimpleCodec`] built from closures.
pub struct ClosureCodec<T> {
    encoder: Arc<Encoder<T>>,
    decoder: Arc<Decoder<T>>,
}

impl<T> Clone for ClosureCodec<T> {
    fn clone(&self) -> Self {
        Self {
            encoder: Arc::clone(&self.encoder),
            decoder: Arc::clone(&self.decoder),
        }
    }
}

impl<T> std::fmt::Debug for ClosureCodec<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ClosureCodec")
    }
}

impl<T> ClosureCodec<T> {
    /// Builds a codec from closures (`JCodec.of`).
    pub fn new<Enc, Dec>(encoder: Enc, decoder: Dec) -> Self
    where
        Enc: Fn(&T) -> String + Send + Sync + 'static,
        Dec: Fn(&str) -> Result<T, String> + Send + Sync + 'static,
    {
        Self {
            encoder: Arc::new(encoder),
            decoder: Arc::new(decoder),
        }
    }
}

impl<T: 'static> SimpleCodec<T> for ClosureCodec<T> {
    fn encode(&self, value: &T) -> String {
        (self.encoder)(value)
    }

    fn decode(&self, json: &str) -> Result<T, String> {
        (self.decoder)(json)
    }
}

/// A [`SimpleCodec`] seen as a storage [`Codec`] (`jsonb`).
#[derive(Clone, Debug)]
pub struct CodecAdapter<C>(pub C);

impl<T, C: SimpleCodec<T>> Codec<T> for CodecAdapter<C> {
    fn format(&self) -> PayloadFormat {
        PayloadFormat::Jsonb
    }

    fn encode(&self, value: &T) -> Result<Vec<u8>, CodecError> {
        Ok(self.0.encode(value).into_bytes())
    }

    fn decode(&self, bytes: &[u8]) -> Result<T, CodecError> {
        let json = std::str::from_utf8(bytes).map_err(|e| CodecError::Decode(e.to_string()))?;
        self.0.decode(json).map_err(CodecError::Decode)
    }
}
