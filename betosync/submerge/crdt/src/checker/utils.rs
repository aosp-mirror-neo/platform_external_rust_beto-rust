// Copyright 2024 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Helper functions for implementing the CRDT checker.

use std::{
    collections::BTreeSet,
    hash::{BuildHasherDefault, DefaultHasher},
};

/// Extension functions on an iterator.
pub trait IterExt<T>: Iterator<Item = T> + Sized {
    /// Whether all elements in the iterator are unique, as determined by `Ord`.
    fn is_uniq(&mut self) -> bool
    where
        T: Ord,
    {
        let mut set = BTreeSet::new();
        self.all(|t| set.insert(t))
    }

    /// Iterate through every pair of elements `(a, b)` in the iterator where `a` is not `b`.
    ///
    /// The returned pair are treated as unordered, which means that the returned iterator will
    /// contain `(a, b)` but not `(b, a)` since they are considered equivalent. This function
    /// requires `T: Copy`, which in many cases can be satisfied by iterating on shared references.
    fn iter_pairs_unordered(&mut self) -> impl Iterator<Item = (T, T)>
    where
        T: Copy,
    {
        let values = self.collect::<Vec<_>>();
        let len = values.len();
        let indices = (0..len).flat_map(move |i| ((i + 1)..len).map(move |j| (i, j)));
        indices.map(move |(i, j)| (values[i], values[j]))
    }
}

impl<T, I: Iterator<Item = T>> IterExt<T> for I {}

/// A [`BuildHasher`][std::hash::BuildHasher] implementation that is deterministic.
///
/// This can be used with [`HashSet`][std::collections::HashSet],
/// [`HashMap`][std::collections::HashMap], or other types that that takes a `BuildHasher` as the
/// type parameter, to replace the default [`RandomState`][std::collections::hash_map::RandomState]
/// that has randomness built-in for HashDos prevention.
///
/// This is especially useful in instrumentation-guided fuzzing where non-determinism will make
/// fuzzing inefficient.
pub type DeterministicHasher = BuildHasherDefault<DefaultHasher>;

#[cfg(test)]
mod tests {
    use std::hash::BuildHasher;

    use super::{DeterministicHasher, IterExt};

    #[test]
    fn test_iter_pairs() {
        let arr = [1, 2, 3, 4];
        let pairs: Vec<_> = arr.iter().iter_pairs_unordered().collect();
        assert_eq!(
            vec![(&1, &2), (&1, &3), (&1, &4), (&2, &3), (&2, &4), (&3, &4),],
            pairs
        );
    }

    #[test]
    fn test_iter_pairs_empty() {
        let pairs: Vec<_> = [0_u8; 0].iter().iter_pairs_unordered().collect();
        assert_eq!(Vec::<(&u8, &u8)>::new(), pairs);
    }

    #[test]
    fn test_is_uniq() {
        assert!([1, 2, 3, 4, 5].into_iter().is_uniq());
        assert!(![1, 5, 3, 5, 2].into_iter().is_uniq());
    }

    #[test]
    fn test_deterministic_hasher() {
        let build_hasher1 = DeterministicHasher::default();
        let build_hasher2 = DeterministicHasher::default();
        assert_eq!(
            build_hasher1.hash_one(b"hello"),
            build_hasher2.hash_one(b"hello")
        );
    }
}
