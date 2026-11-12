//! The key order of a parsed JSON document, kept so the document serialises back in that order.
//!
//! The derived serde forms write struct fields in declaration order, which is the TypeSafe
//! schema's property order. Documented examples do not always use it: the images guide puts
//! `assets` before `questions`, and the schema's property examples list keys alphabetically
//! (`{"false": .., "true": ..}`, `{"instructions": .., "type": "noul"}`). A root type parsed by
//! its `from_json` records the key order of the whole document as a [`KeyOrder`] tree and
//! re-applies it when serialised, so every object in the document, nested ones included,
//! comes back in the order it arrived. Values come from the typed fields only; the tree holds
//! key names, never values.
//!
//! The typed parse runs first and is unchanged (duplicate keys refused, field paths and
//! line/column kept); the order is read in a second parse of the same text. A value built in
//! code, or parsed through `serde::Deserialize` directly, has an empty tree and serialises in
//! schema order.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, ser};

/// The key order of one JSON value and of everything inside it.
#[derive(Clone, Default, PartialEq, Eq)]
pub(crate) struct KeyOrder {
    /// An object's keys in document order, each with the order inside its value.
    keys: Vec<(String, KeyOrder)>,
    /// An array's items' orders, by index.
    items: Vec<KeyOrder>,
}

impl fmt::Debug for KeyOrder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            formatter.write_str("KeyOrder(schema)")
        } else {
            formatter.write_str("KeyOrder(document)")
        }
    }
}

impl KeyOrder {
    /// The key order of `value`.
    pub(crate) fn of(value: &serde_json::Value) -> Self {
        match value {
            serde_json::Value::Object(map) => Self {
                keys: map
                    .iter()
                    .map(|(key, child)| (key.clone(), Self::of(child)))
                    .collect(),
                items: Vec::new(),
            },
            serde_json::Value::Array(items) => Self {
                keys: Vec::new(),
                items: items.iter().map(Self::of).collect(),
            },
            _ => Self::default(),
        }
    }

    fn is_empty(&self) -> bool {
        self.keys.is_empty() && self.items.is_empty()
    }

    /// Reorders `value`'s objects to this order. Keys the order does not name (added after
    /// parsing) follow, in their serialised order.
    fn apply(&self, value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) if !self.keys.is_empty() => {
                let old = std::mem::take(map);
                let serialised: Vec<String> = old.keys().cloned().collect();
                let mut values: HashMap<String, serde_json::Value> = old.into_iter().collect();
                for (key, child) in &self.keys {
                    if let Some(mut child_value) = values.remove(key) {
                        child.apply(&mut child_value);
                        map.insert(key.clone(), child_value);
                    }
                }
                for key in serialised {
                    if let Some(rest) = values.remove(&key) {
                        map.insert(key, rest);
                    }
                }
            }
            serde_json::Value::Array(items) => {
                for (item, child) in items.iter_mut().zip(&self.items) {
                    child.apply(item);
                }
            }
            _ => {}
        }
    }
}

/// A root type whose document key order is kept: its derived (schema-order) serde forms and
/// its recorded order.
pub(crate) trait Ordered: Sized {
    /// The derived deserializer (`#[serde(remote = "Self")]`), schema order.
    fn wire_deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error>;
    /// The derived serializer (`#[serde(remote = "Self")]`), schema order.
    fn wire_serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>;
    /// The recorded order.
    fn key_order(&self) -> &KeyOrder;
    /// Sets the recorded order.
    fn set_key_order(&mut self, order: KeyOrder);
}

/// Deserializes through [`Ordered::wire_deserialize`].
struct Typed<T>(T);

impl<'de, T: Ordered> Deserialize<'de> for Typed<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        T::wire_deserialize(deserializer).map(Self)
    }
}

/// Serializes through [`Ordered::wire_serialize`].
struct Wire<'a, T>(&'a T);

impl<T: Ordered> Serialize for Wire<'_, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.wire_serialize(serializer)
    }
}

/// Serializes `value` in its recorded order (schema order when none is recorded).
pub(crate) fn serialize<T: Ordered, S: Serializer>(
    value: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let order = value.key_order();
    if order.is_empty() {
        return value.wire_serialize(serializer);
    }
    let mut tree = serde_json::to_value(Wire(value)).map_err(ser::Error::custom)?;
    order.apply(&mut tree);
    tree.serialize(serializer)
}

/// A failed [`parse`]: the field path (`.` at the root) and the full serde_json message.
pub(crate) struct ParseError {
    pub(crate) path: String,
    pub(crate) message: String,
}

impl ParseError {
    /// The message, prefixed by the path when it is not the root (`answers.q: missing field ..`).
    pub(crate) fn located(&self) -> String {
        if self.path == "." {
            self.message.clone()
        } else {
            format!("{}: {}", self.path, self.message)
        }
    }

    fn at_root(error: &serde_json::Error) -> Self {
        Self {
            path: ".".to_owned(),
            message: error.to_string(),
        }
    }
}

/// Parses `text` with the derived deserializer under serde_path_to_error, then records its
/// key order.
///
/// # Errors
///
/// The typed parse's error, with its field path; trailing text after the value is refused.
pub(crate) fn parse<T: Ordered>(text: &[u8]) -> Result<T, ParseError> {
    let mut deserializer = serde_json::Deserializer::from_slice(text);
    let Typed(mut value) = serde_path_to_error::deserialize::<_, Typed<T>>(&mut deserializer)
        .map_err(|error| ParseError {
            path: error.path().to_string(),
            message: error.inner().to_string(),
        })?;
    deserializer
        .end()
        .map_err(|error| ParseError::at_root(&error))?;
    let tree: serde_json::Value =
        serde_json::from_slice(text).map_err(|error| ParseError::at_root(&error))?;
    value.set_key_order(KeyOrder::of(&tree));
    Ok(value)
}
