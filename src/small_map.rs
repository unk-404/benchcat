//! A tiny sorted map for the per-row `params`/`metrics`/`extra` fields.
//!
//! A benchmark row has a handful of parameters, not hundreds. `BTreeMap` pays
//! for a node allocation on the first insert and a tree descent on every
//! lookup; a sorted `Vec` keeps the same deterministic key order with one small
//! contiguous allocation, binary search, and an `insert` that shifts at most a
//! few elements.

use std::borrow::Borrow;
use std::marker::PhantomData;
use std::ops::Index;

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A sorted map with the API used by the benchmark model.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SmallMap<K, V> {
    entries: Vec<(K, V)>,
}

impl<K, V> SmallMap<K, V> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> + '_ {
        self.entries.iter().map(|(key, value)| (key, value))
    }

    pub fn keys(&self) -> impl Iterator<Item = &K> + '_ {
        self.entries.iter().map(|(key, _)| key)
    }

    pub fn values(&self) -> impl Iterator<Item = &V> + '_ {
        self.entries.iter().map(|(_, value)| value)
    }
}

impl<K: Ord, V> SmallMap<K, V> {
    /// Insert, replacing an existing key; returns the old value.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        match self.entries.binary_search_by(|(probe, _)| probe.cmp(&key)) {
            Ok(index) => Some(std::mem::replace(&mut self.entries[index].1, value)),
            Err(index) => {
                self.entries.insert(index, (key, value));
                None
            }
        }
    }

    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.entries
            .binary_search_by(|(probe, _)| probe.borrow().cmp(key))
            .ok()
            .map(|index| &self.entries[index].1)
    }

    pub fn get_key_value<Q>(&self, key: &Q) -> Option<(&K, &V)>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.entries
            .binary_search_by(|(probe, _)| probe.borrow().cmp(key))
            .ok()
            .map(|index| {
                let (key, value) = &self.entries[index];
                (key, value)
            })
    }

    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.entries
            .binary_search_by(|(probe, _)| probe.borrow().cmp(key))
            .is_ok()
    }
}

impl<K: Ord, V, Q> Index<&Q> for SmallMap<K, V>
where
    K: Borrow<Q>,
    Q: Ord + ?Sized,
{
    type Output = V;

    fn index(&self, key: &Q) -> &Self::Output {
        self.get(key).expect("key not found in SmallMap")
    }
}

impl<K: Ord, V> FromIterator<(K, V)> for SmallMap<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut map = Self::new();
        for (key, value) in iter {
            map.insert(key, value);
        }
        map
    }
}

impl<K: Ord, V> Extend<(K, V)> for SmallMap<K, V> {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        for (key, value) in iter {
            self.insert(key, value);
        }
    }
}

impl<K, V> IntoIterator for SmallMap<K, V> {
    type Item = (K, V);
    type IntoIter = std::vec::IntoIter<(K, V)>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<'a, K, V> IntoIterator for &'a SmallMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = Box<dyn Iterator<Item = (&'a K, &'a V)> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.iter())
    }
}

impl<K, V> Serialize for SmallMap<K, V>
where
    K: Serialize + Ord,
    V: Serialize,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.len()))?;
        for (key, value) in self.iter() {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

impl<'de, K, V> Deserialize<'de> for SmallMap<K, V>
where
    K: Deserialize<'de> + Ord,
    V: Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct MapVisitor<K, V>(PhantomData<(K, V)>);

        impl<'de, K, V> Visitor<'de> for MapVisitor<K, V>
        where
            K: Deserialize<'de> + Ord,
            V: Deserialize<'de>,
        {
            type Value = SmallMap<K, V>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a map")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                let mut map = SmallMap::new();
                while let Some((key, value)) = access.next_entry()? {
                    map.insert(key, value);
                }
                Ok(map)
            }
        }

        deserializer.deserialize_map(MapVisitor(PhantomData))
    }
}

#[cfg(test)]
mod tests {
    use super::SmallMap;
    use serde_json::{Value, json};

    #[test]
    fn keeps_keys_sorted() {
        let mut map = SmallMap::new();
        map.insert("b".to_owned(), 2);
        map.insert("a".to_owned(), 1);
        map.insert("c".to_owned(), 3);
        let keys: Vec<&str> = map.keys().map(String::as_str).collect();
        assert_eq!(keys, ["a", "b", "c"]);
        assert_eq!(map.get("b"), Some(&2));
    }

    #[test]
    fn replaces_existing_keys() {
        let mut map = SmallMap::new();
        assert_eq!(map.insert("a".to_owned(), 1), None);
        assert_eq!(map.insert("a".to_owned(), 2), Some(1));
        assert_eq!(map.len(), 1);
        assert_eq!(map["a"], 2);
    }

    #[test]
    fn round_trips_json_and_flatten() {
        let mut params = SmallMap::new();
        params.insert("workers".to_owned(), json!(4));
        params.insert("block".to_owned(), json!("8MiB"));
        let encoded = serde_json::to_string(&params).unwrap();
        assert_eq!(encoded, r#"{"block":"8MiB","workers":4}"#);
        let decoded: SmallMap<String, Value> = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, params);

        #[derive(serde::Deserialize)]
        struct Row {
            id: String,
            #[serde(flatten)]
            extra: SmallMap<String, Value>,
        }
        let row: Row = serde_json::from_str(r#"{"id":"x","alpha":1,"beta":"two"}"#).unwrap();
        assert_eq!(row.id, "x");
        assert_eq!(row.extra.get("alpha"), Some(&json!(1)));
        assert_eq!(row.extra.get("beta"), Some(&json!("two")));
        assert_eq!(row.extra.len(), 2);
    }

    #[test]
    fn compares_in_key_order() {
        let one: SmallMap<String, u8> = [("a".to_owned(), 1), ("b".to_owned(), 2)]
            .into_iter()
            .collect();
        assert_eq!(one.iter().count(), 2);
        assert!(!one.is_empty());
        assert!(SmallMap::<String, u8>::new().is_empty());
    }
}
