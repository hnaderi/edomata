//! Port of `JCodecSuite.scala`.

use edomata_backend::{Codec, CodecError, PayloadFormat};
use edomata_simple::{ClosureCodec, CodecAdapter, SimpleCodec, serde_codec};

#[test]
fn of_creates_codec_from_closures() {
    let codec = ClosureCodec::<i32>::new(
        |i| i.to_string(),
        |s| s.parse::<i32>().map_err(|e| e.to_string()),
    );
    assert_eq!(codec.encode(&42), "42");
    assert_eq!(codec.decode("99"), Ok(99));
}

#[test]
fn into_codec_produces_a_jsonb_storage_codec() {
    let codec = ClosureCodec::<String>::new(
        |s| format!("\"{s}\""),
        |json| Ok(json.trim_matches('"').to_string()),
    );
    let adapter = CodecAdapter(codec.clone());
    assert_eq!(adapter.format(), PayloadFormat::Jsonb);
    assert_eq!(
        adapter.encode(&"hi".to_string()).unwrap(),
        b"\"hi\"".to_vec()
    );
    assert_eq!(adapter.decode(b"\"hi\"").unwrap(), "hi");
    let sqlx_codec = codec.into_codec();
    assert_eq!(sqlx_codec.format(), PayloadFormat::Jsonb);
    assert_eq!(sqlx_codec.sql_type(), "jsonb");
}

#[test]
fn into_codec_decode_failure_returns_the_message() {
    let codec = ClosureCodec::<i32>::new(|i| i.to_string(), |_| Err("parse error".to_string()));
    let adapter = CodecAdapter(codec);
    assert_eq!(adapter.format(), PayloadFormat::Jsonb);
    assert_eq!(
        adapter.decode(b"1"),
        Err(CodecError::Decode("parse error".to_string()))
    );
    assert!(matches!(
        adapter.decode(&[0xff, 0xfe]),
        Err(CodecError::Decode(_))
    ));
}

#[test]
fn serde_codec_is_jsonb() {
    let codec = serde_codec::<Vec<i32>>();
    assert_eq!(codec.format(), PayloadFormat::Jsonb);
}
