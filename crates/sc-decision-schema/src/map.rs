fn duplicate_message(key: &dyn fmt::Display) -> String {
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
/// use sc_decision_schema::jev::v1::OrderedMap;
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
    /// `INPUT_INVALID` when two pairs share a key; the fix hint names it.
    pub fn try_from_pairs(pairs: Vec<(K, V)>) -> Result<Self, JevError> {
        match duplicate_key(&pairs) {
            Some(key) => Err(JevError::input_invalid(duplicate_message(key))),
            None => Ok(Self(pairs)),
        }
    }
}

impl<K, V> OrderedMap<K, V> {
    /// The pairs, in order.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> + '_ {
        self.0.iter().map(|(key, value)| (key, value))
    }

    /// The keys, in order.
    pub fn keys(&self) -> impl Iterator<Item = &K> + '_ {
        self.0.iter().map(|(key, _)| key)
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

impl<K: PartialEq, V> OrderedMap<K, V> {
    /// The value of `key`, if present.
    #[must_use]
    pub fn get(&self, key: &K) -> Option<&V> {
        self.0
            .iter()
            .find_map(|(candidate, value)| (candidate == key).then_some(value))
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

/// A JSON object read in document order, with duplicates kept. Raw parse
/// types hold it so their owners can refuse duplicates with a typed error.
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
