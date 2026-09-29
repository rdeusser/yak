/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Hash function abstractions for yak.
//!
//! This crate provides a level of indirection for hash function implementations
//! used throughout yak. The default hasher used by Rust's `HashMap` is
//! optimized for security rather than speed. For internal hash maps where
//! security is not a concern, using this crate's hasher provides better
//! performance.
//!
//! # Usage
//!
//! ```
//! use yak_hash::YakMutMap;
//! use yak_hash::YakMutSet;
//!
//! let mut map: YakMutMap<String, i32> = YakMutMap::default();
//! map.insert("key".to_string(), 42);
//!
//! let mut set: YakMutSet<i32> = YakMutSet::default();
//! set.insert(42);
//! ```
//!
//! For ordered collections that preserve insertion order, use [`YakIndexMap`]
//! and [`YakIndexSet`]:
//!
//! ```
//! use yak_hash::YakIndexMap;
//! use yak_hash::YakIndexSet;
//!
//! let mut map: YakIndexMap<String, i32> = YakIndexMap::default();
//! map.insert("first".to_string(), 1);
//! map.insert("second".to_string(), 2);
//! // Iteration order is guaranteed: "first", then "second"
//!
//! let mut set: YakIndexSet<i32> = YakIndexSet::default();
//! set.insert(42);
//! ```

use std::hash::BuildHasher;
use std::hash::Hasher;

use dupe::Dupe;
use fxhash::FxHasher64;

/// A hasher for yak internal use.
///
/// This hasher is optimized for speed rather than security, making it suitable
/// for internal hash maps where hash-flooding attacks are not a concern.
///
/// Currently wraps `fxhash::FxHasher64`.
///
/// # Important
///
/// When wrapping this hasher, all `write_*` methods should be explicitly
/// forwarded to the inner hasher. This is not a correctness issue — both paths
/// produce valid hashes — but it is a performance concern. The default `Hasher`
/// trait implementations of `write_u8`, `write_usize`, etc. serialize the value
/// to bytes and call `self.write()`, but `FxHasher` processes typed writes more
/// efficiently (e.g., `FxHasher::write_u64` hashes the value directly as a word,
/// whereas `FxHasher::write(&[u8; 8])` processes it byte-by-byte). If a
/// forwarding method is omitted, calls to it will silently take the slower
/// byte-serialization path.
#[derive(Default)]
pub struct YakHasher(FxHasher64);

impl YakHasher {
    /// Creates a new hasher.
    #[inline]
    pub fn new() -> Self {
        YakHasher::default()
    }
}

// IMPORTANT: `Hasher` wrappers must explicitly forward every `write_*` method
// to the inner hasher. See the doc comment on `YakHasher` for details.
#[allow(clippy::missing_trait_methods)]
impl Hasher for YakHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0.finish()
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        self.0.write(bytes)
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.0.write_u8(i)
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.0.write_u16(i)
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.0.write_u32(i)
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.0.write_u64(i)
    }

    #[inline]
    fn write_u128(&mut self, i: u128) {
        self.0.write_u128(i)
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.0.write_usize(i)
    }
}

/// [`BuildHasher`] implementation which produces [`YakHasher`].
#[derive(Default, Debug, Clone, Copy, Dupe)]
pub struct YakHasherBuilder;

impl BuildHasher for YakHasherBuilder {
    type Hasher = YakHasher;

    #[inline]
    fn build_hasher(&self) -> Self::Hasher {
        YakHasher::new()
    }
}

/// A [`HashMap`](std::collections::HashMap) using [`YakHasherBuilder`].
///
/// The default map for yak code: mutable, with no guarantee about iteration order. The
/// hasher is faster than the standard library's, which is optimized against hash flooding -
/// not a concern for yak's internal maps.
pub type YakMutMap<K, V> = std::collections::HashMap<K, V, YakHasherBuilder>;

/// A [`HashSet`](std::collections::HashSet) using [`YakHasherBuilder`].
///
/// The set counterpart of [`YakMutMap`], and the default set for yak code.
pub type YakMutSet<K> = std::collections::HashSet<K, YakHasherBuilder>;

/// An [`IndexMap`](indexmap::IndexMap) using the default hasher.
///
/// This is a type alias for `indexmap::IndexMap` that preserves insertion order.
/// Unlike [`YakMutMap`], iteration order is guaranteed to match insertion order.
///
/// This abstraction allows the hasher implementation to be changed centrally
/// in a future commit.
pub type YakIndexMap<K, V> = indexmap::IndexMap<K, V>;

/// An [`IndexSet`](indexmap::IndexSet) using the default hasher.
///
/// This is a type alias for `indexmap::IndexSet` that preserves insertion order.
/// Unlike [`YakMutSet`], iteration order is guaranteed to match insertion order.
///
/// This abstraction allows the hasher implementation to be changed centrally
/// in a future commit.
pub type YakIndexSet<K> = indexmap::IndexSet<K>;

/// Creates a [`YakIndexMap`] from a list of key-value pairs.
///
/// This macro mirrors the `indexmap!` macro from the `indexmap` crate but uses
/// whatever hasher [`YakIndexMap`] is configured to use, allowing the hasher
/// to be changed centrally.
///
/// # Example
///
/// ```
/// use yak_hash::yak_indexmap;
///
/// let map = yak_indexmap! {
///     "a" => 1,
///     "b" => 2,
/// };
/// assert_eq!(map["a"], 1);
/// assert_eq!(map["b"], 2);
/// ```
#[macro_export]
macro_rules! yak_indexmap {
    () => {
        $crate::YakIndexMap::default()
    };
    ($($key:expr => $value:expr),+ $(,)?) => {{
        let mut map = $crate::YakIndexMap::default();
        $(map.insert($key, $value);)+
        map
    }};
}

/// Creates a [`YakIndexSet`] from a list of values.
///
/// This macro mirrors the `indexset!` macro from the `indexmap` crate but uses
/// whatever hasher [`YakIndexSet`] is configured to use, allowing the hasher
/// to be changed centrally.
///
/// # Example
///
/// ```
/// use yak_hash::yak_indexset;
///
/// let set = yak_indexset![1, 2, 3];
/// assert!(set.contains(&1));
/// assert!(set.contains(&2));
/// assert!(set.contains(&3));
/// ```
#[macro_export]
macro_rules! yak_indexset {
    () => {
        $crate::YakIndexSet::default()
    };
    ($($value:expr),+ $(,)?) => {{
        let mut set = $crate::YakIndexSet::default();
        $(set.insert($value);)+
        set
    }};
}

/// A [`DashMap`](dashmap::DashMap) using [`YakHasherBuilder`].
///
/// The concurrent map for yak code. Construct it with `default()`; `DashMap::new()` exists
/// only for the standard library's hasher.
pub type YakDashMap<K, V, S = YakHasherBuilder> = dashmap::DashMap<K, V, S>;

/// A [`DashSet`](dashmap::DashSet) using [`YakHasherBuilder`].
///
/// The set counterpart of [`YakDashMap`].
pub type YakDashSet<K, S = YakHasherBuilder> = dashmap::DashSet<K, S>;

/// Do not use in new code. A [`HashMap`](std::collections::HashMap) with the standard
/// library's `RandomState` hasher.
///
/// What is left of this alias marks the retained maps - dice values, daemon state, the
/// materializer tree - that are waiting on the immutable map type from the retained-maps
/// plan. Once those are converted this alias goes away. Anywhere else, the map is either
/// transient, and wants [`YakMutMap`], or forced to be a concrete `HashMap` by something
/// outside yak, and wants [`IntentionallyStdHashMap`].
pub type StdYakHashMap<K, V> = std::collections::HashMap<K, V>;

/// A [`HashMap`](std::collections::HashMap) that intentionally uses the standard library's
/// default `RandomState` hasher rather than yak's performance-optimized hasher.
///
/// Use this at API boundaries where the concrete type `HashMap<K, V>` is required —
/// for example, protobuf-generated struct fields, third-party crate APIs, or rusqlite
/// result collection. In all other cases, prefer [`YakMutMap`].
pub type IntentionallyStdHashMap<K, V> = std::collections::HashMap<K, V>;

/// A [`HashSet`](std::collections::HashSet) that intentionally uses the standard library's
/// default `RandomState` hasher rather than yak's performance-optimized hasher.
///
/// Use this at API boundaries where the concrete type `HashSet<K>` is required —
/// for example, starlark's `ast.lint()` API which expects `&HashSet<String>`.
/// In all other cases, prefer [`YakMutSet`].
pub type IntentionallyStdHashSet<K> = std::collections::HashSet<K>;

#[cfg(test)]
mod tests {
    use std::hash::Hash;
    use std::hash::Hasher;

    use super::*;

    fn hash_with_yak_hasher<T: Hash>(value: &T) -> u64 {
        let mut hasher = YakHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn test_yak_hasher_deterministic() {
        let h1 = hash_with_yak_hasher(&42u64);
        let h2 = hash_with_yak_hasher(&42u64);
        assert_eq!(h1, h2, "YakHasher should be deterministic");
    }

    #[test]
    fn test_yak_hasher_different_values() {
        let h1 = hash_with_yak_hasher(&42u64);
        let h2 = hash_with_yak_hasher(&43u64);
        assert_ne!(h1, h2, "Different values should produce different hashes");
    }

    #[test]
    fn test_yak_mut_map() {
        let mut map: YakMutMap<String, i32> = YakMutMap::default();
        map.insert("key1".to_owned(), 1);
        map.insert("key2".to_owned(), 2);

        assert_eq!(map.get("key1"), Some(&1));
        assert_eq!(map.get("key2"), Some(&2));
        assert_eq!(map.get("key3"), None);
    }

    #[test]
    fn test_yak_mut_set() {
        let mut set: YakMutSet<i32> = YakMutSet::default();
        set.insert(1);
        set.insert(2);
        set.insert(1);

        assert_eq!(set.len(), 2);
        assert!(set.contains(&1));
        assert!(set.contains(&2));
        assert!(!set.contains(&3));
    }

    #[test]
    fn test_yak_index_map() {
        let mut map: YakIndexMap<String, i32> = YakIndexMap::default();
        map.insert("first".to_owned(), 1);
        map.insert("second".to_owned(), 2);
        map.insert("third".to_owned(), 3);

        assert_eq!(map.get("first"), Some(&1));
        assert_eq!(map.get("second"), Some(&2));
        assert_eq!(map.get("third"), Some(&3));
        assert_eq!(map.get("fourth"), None);

        // Verify insertion order is preserved
        let keys: Vec<_> = map.keys().collect();
        assert_eq!(keys, vec!["first", "second", "third"]);
    }

    #[test]
    fn test_yak_index_set() {
        let mut set: YakIndexSet<i32> = YakIndexSet::default();
        set.insert(3);
        set.insert(1);
        set.insert(2);
        set.insert(1); // duplicate

        assert_eq!(set.len(), 3);
        assert!(set.contains(&1));
        assert!(set.contains(&2));
        assert!(set.contains(&3));
        assert!(!set.contains(&4));

        // Verify insertion order is preserved
        let values: Vec<_> = set.iter().copied().collect();
        assert_eq!(values, vec![3, 1, 2]);
    }

    #[test]
    fn test_multi_write_sequence() {
        let mut h1 = YakHasher::new();
        h1.write_u64(1);
        h1.write_u64(2);

        let mut h2 = YakHasher::new();
        h2.write_u64(1);
        h2.write_u64(2);

        assert_eq!(
            h1.finish(),
            h2.finish(),
            "Identical multi-write sequences should produce identical hashes"
        );
    }
}
