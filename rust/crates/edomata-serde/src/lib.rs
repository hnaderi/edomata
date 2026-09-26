//! # Edomata serde codecs
//!
//! The one payload codec of the Rust port, replacing the Circe, jsoniter and
//! uPickle codec modules: [`SerdeCodec<T>`] implements
//! [`edomata_backend::Codec`] for any `T: Serialize + DeserializeOwned` with
//! `serde_json`.
//!
//! Payloads are stored as **`jsonb` by default** (queryable with `->`, `@>`
//! and GIN indexes); `json` and `bytea` are supported for tables that already
//! use them. JSON is written straight to bytes (`serde_json::to_writer`)
//! and, with the `sqlx` feature, the [`pg`] module puts those bytes on the
//! wire in PostgreSQL's binary `jsonb` format (version byte + UTF-8 JSON)
//! without any intermediate `String` or `serde_json::Value`.
//!
//! ```
//! use edomata_backend::{Codec, PayloadFormat};
//! use edomata_serde::SerdeCodec;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, PartialEq, Serialize, Deserialize)]
//! enum Event { Opened { owner: String }, Closed }
//!
//! let codec = SerdeCodec::<Event>::jsonb();
//! assert_eq!(codec.format(), PayloadFormat::Jsonb);
//! let bytes = codec.encode(&Event::Opened { owner: "bob".into() }).unwrap();
//! assert_eq!(bytes, br#"{"Opened":{"owner":"bob"}}"#);
//! assert_eq!(codec.decode(&bytes).unwrap(), Event::Opened { owner: "bob".into() });
//! ```
//!
//! ## Reading payloads written by Scala
//!
//! Circe (generic derivation), jsoniter-scala (including its `msgpack` codec,
//! which writes JSON bytes) and uPickle JSON payloads are all JSON and can be
//! read by choosing the matching `serde` enum representation:
//!
//! | Scala codec | JSON shape of an ADT case | serde representation |
//! |-------------|---------------------------|----------------------|
//! | Circe `generic.auto` | `{"Opened":{...}}`, `{"Closed":{}}`, `None` → `null` | default (externally tagged); parameterless cases as `Closed {}` |
//! | jsoniter `JsonCodecMaker.make` | `{"type":"Opened",...}`, `{"type":"Closed"}`, `None` omitted | `#[serde(tag = "type")]` plus `#[serde(default, skip_serializing_if = "Option::is_none")]` on `Option` fields |
//! | uPickle `ReadWriter.derived` | `{"$type":"Opened",...}`, `"Closed"`, `None` → `[]` / `Some(x)` → `[x]` | `#[serde(tag = "$type")]` inside an `#[serde(untagged)]` wrapper for bare-string cases, plus [`compat::upickle_option`] |
//!
//! The golden payload tests (`tests/golden_payloads.rs`) check these shapes
//! against files written by the Scala codecs. uPickle's `msgpack` codec
//! writes real MessagePack into `bytea`; those payloads cannot be read
//! because `serde_json` is the only serializer.

#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod codec;
pub mod compat;
#[cfg(feature = "sqlx")]
#[cfg_attr(docsrs, doc(cfg(feature = "sqlx")))]
pub mod pg;

pub use codec::SerdeCodec;
pub use edomata_backend::{Codec, CodecError, PayloadFormat};
