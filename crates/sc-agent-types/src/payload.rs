//! Canonical judge payload bytes (ADR-SAG-0002).
//!
//! # Encoding
//!
//! The canonical payload is compact JSON encoded as UTF-8:
//!
//! - Objects use [`std::collections::BTreeMap`] key order (lexicographic UTF-8).
//!   [`serde_json::Map`] in this crate is that map because `serde_json` is
//!   built without `preserve_order`.
//! - [`canonical_json_bytes`] still walks objects and reinserts keys in sorted
//!   order so the byte contract does not depend on map iteration.
//! - Strings use `serde_json`'s default escapes (`\"`, `\\`, and `\u00XX` for
//!   controls). Other characters are raw UTF-8, not `\u` escapes.
//! - Arrays keep their element order. Numbers, booleans, and null use
//!   `serde_json`'s compact forms. No extra whitespace is emitted.
//!
//! `prompt_sha256` is SHA-256 of those bytes. LLM system-prompt text is not
//! included.

use serde_json::{Map, Value};

use crate::error::JevError;
use crate::judge::PayloadBundle;

/// Encodes `value` as canonical judge-payload bytes.
///
/// # Errors
///
/// Returns [`JevError::InputInvalid`] when `serde_json` cannot encode `value`.
///
/// # Examples
///
/// ```
/// let value = serde_json::json!({"b": 1, "a": {"d": true, "c": "ok"}});
/// let bytes = sc_agent_types::canonical_json_bytes(&value).expect("finite JSON");
/// assert_eq!(bytes, br#"{"a":{"c":"ok","d":true},"b":1}"#);
/// ```
pub fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>, JevError> {
    serde_json::to_vec(&sorted(value)).map_err(|error| JevError::InputInvalid {
        detail: format!("canonical JSON encoding failed: {error}"),
    })
}

/// Builds a [`PayloadBundle`] from a JSON value.
///
/// # Errors
///
/// Returns [`JevError::InputInvalid`] when the value cannot be encoded.
pub fn payload_bundle_from_value(value: &Value) -> Result<PayloadBundle, JevError> {
    Ok(PayloadBundle::from_canonical_bytes(canonical_json_bytes(
        value,
    )?))
}

fn sorted(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut ordered = Map::new();
            for key in keys {
                let child = map.get(key).expect("key was taken from this map");
                ordered.insert(key.clone(), sorted(child));
            }
            Value::Object(ordered)
        }
        Value::Array(items) => Value::Array(items.iter().map(sorted).collect()),
        other => other.clone(),
    }
}
