//! [`OrderedMap`]: a JSON object kept in document order that refuses duplicate keys.
//!
//! Question ids, answer ids, choice criteria and probabilities are JSON objects whose key
//! order is part of the wire form (the answers come back in request order). `OrderedMap`
//! keeps that order through a round trip and refuses a key given twice, both when parsing
//! and when built from pairs.

use std::borrow::Borrow;
use std::collections::BTreeSet;
use std::fmt;
use std::marker::PhantomData;

use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::DecisionError;

/// The message of a refused duplicate key, shared by parsing and [`DecisionError::DuplicateKey`].
pub(crate) fn duplicate_message(key: &dyn fmt::Display) -> String {
    format!("duplicate key `{key}`; give every key once")
}

/// A map serialized as a JSON object whose key order is the `Vec` order, and
/// deserialized in document order.
///
/// A duplicate key is an error that names the key. Use it wherever the wire
/// form's key order matters (question ids, criteria options, top-level state
/// keys).
///
/// # Examples
///
/// ```
/// use sc_decision_schema::OrderedMap;
///
/// let map: OrderedMap<String, u8> = serde_json::from_str(r#"{"zeta":1,"alpha":2}"#)?;
/// assert_eq!(serde_json::to_string(&map)?, r#"{"zeta":1,"alpha":2}"#);
/// assert!(serde_json::from_str::<OrderedMap<String, u8>>(r#"{"a":1,"a":2}"#).is_err());
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct OrderedMap<K, V>(Vec<(K, V)>);

impl<K: Ord + fmt::Display, V> OrderedMap<K, V> {
    /// A map of `pairs`, in that order.
    ///
    /// # Errors
    ///
    /// [`DecisionError::DuplicateKey`] naming the key when two pairs share it.
    pub fn try_from_pairs(pairs: Vec<(K, V)>) -> Result<Self, DecisionError> {
        match duplicate_key(&pairs) {
            Some(key) => Err(DecisionError::DuplicateKey {
                key: key.to_string(),
            }),
            None => Ok(Self(pairs)),
        }
    }
}

impl<K, V> OrderedMap<K, V> {
    /// The pairs, in order.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> + '_ {
        self.0.iter().map(|(key, value)| (key, value))
    }

    /// The number of pairs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are no pairs.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<K, V> OrderedMap<K, V> {
    /// The value of `key`, if present; `key` may be any borrowed form (`map.get("q1")`).
    #[must_use]
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: ?Sized + PartialEq,
    {
        self.0
            .iter()
            .find_map(|(candidate, value)| (candidate.borrow() == key).then_some(value))
    }
}

/// The pairs of an [`OrderedMap`] as `(&key, &value)`, in order.
type PairRefs<'a, K, V> =
    std::iter::Map<std::slice::Iter<'a, (K, V)>, fn(&'a (K, V)) -> (&'a K, &'a V)>;

fn pair_refs<K, V>((key, value): &(K, V)) -> (&K, &V) {
    (key, value)
}

impl<'a, K, V> IntoIterator for &'a OrderedMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = PairRefs<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.0
            .iter()
            .map(pair_refs as fn(&'a (K, V)) -> (&'a K, &'a V))
    }
}

fn duplicate_key<K: Ord + fmt::Display, V>(pairs: &[(K, V)]) -> Option<&K> {
    let mut seen = BTreeSet::new();
    pairs
        .iter()
        .map(|(key, _)| key)
        .find(|key| !seen.insert(*key))
}

impl<K: Serialize, V: Serialize> Serialize for OrderedMap<K, V> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

/// A JSON object read in document order, with duplicates kept, so the
/// `OrderedMap` deserializer can name the duplicate it refuses.
struct Pairs<K, V>(Vec<(K, V)>);

struct PairsVisitor<K, V>(PhantomData<(K, V)>);

impl<'de, K, V> Visitor<'de> for PairsVisitor<K, V>
where
    K: Deserialize<'de>,
    V: Deserialize<'de>,
{
    type Value = Vec<(K, V)>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
        let mut pairs = Vec::with_capacity(access.size_hint().unwrap_or(0).min(1024));
        while let Some(pair) = access.next_entry::<K, V>()? {
            pairs.push(pair);
        }
        Ok(pairs)
    }
}

impl<'de, K: Deserialize<'de>, V: Deserialize<'de>> Deserialize<'de> for Pairs<K, V> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer
            .deserialize_map(PairsVisitor(PhantomData))
            .map(Self)
    }
}

impl<'de, K, V> Deserialize<'de> for OrderedMap<K, V>
where
    K: Deserialize<'de> + Ord + fmt::Display,
    V: Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let Pairs(pairs) = Pairs::<K, V>::deserialize(deserializer)?;
        match duplicate_key(&pairs) {
            Some(key) => Err(de::Error::custom(duplicate_message(key))),
            None => Ok(Self(pairs)),
        }
    }
}
