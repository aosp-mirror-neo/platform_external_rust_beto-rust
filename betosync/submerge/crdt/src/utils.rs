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

use std::{
    borrow::Borrow,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    hash::{BuildHasher, Hash},
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ZipItem<L, R> {
    Left(L),
    Right(R),
    Both(L, R),
}

pub fn zip_hash_map<'m, K, K1, K2, V1, V2, S>(
    map1: &'m HashMap<K1, V1, S>,
    map2: &'m HashMap<K2, V2, S>,
) -> impl Iterator<Item = (&'m K, ZipItem<&'m V1, &'m V2>)>
where
    K1: Borrow<K> + Hash + Eq,
    K2: Borrow<K> + Hash + Eq,
    K: Eq + Hash + 'm,
    S: BuildHasher + Default,
{
    let keys: HashSet<&K, S> = map1
        .keys()
        .map(|k| k.borrow())
        .chain(map2.keys().map(|k| k.borrow()))
        .collect();
    keys.into_iter()
        .map(|key| match (map1.get(key), map2.get(key)) {
            (Some(value_a), Some(value_b)) => (key, ZipItem::Both(value_a, value_b)),
            (Some(v), None) => (key, ZipItem::Left(v)),
            (None, Some(v)) => (key, ZipItem::Right(v)),
            (None, None) => unreachable!("Iterator was built from a.keys.chain(b.keys)"),
        })
}

pub fn zip_btree_map<'m, K, K1, K2, V1, V2>(
    map1: &'m BTreeMap<K1, V1>,
    map2: &'m BTreeMap<K2, V2>,
) -> impl Iterator<Item = (&'m K, ZipItem<&'m V1, &'m V2>)>
where
    K1: Borrow<K> + Ord,
    K2: Borrow<K> + Ord,
    K: Eq + Ord + 'm,
{
    let keys: BTreeSet<&K> = map1
        .keys()
        .map(|k| k.borrow())
        .chain(map2.keys().map(|k| k.borrow()))
        .collect();
    let mut map1_iter = map1.iter().peekable();
    let mut map2_iter = map2.iter().peekable();
    keys.into_iter().map(move |key| {
        match (
            map1_iter.next_if(|(k, _)| (**k).borrow() == key),
            map2_iter.next_if(|(k, _)| (**k).borrow() == key),
        ) {
            (Some((_, value_a)), Some((_, value_b))) => (key, ZipItem::Both(value_a, value_b)),
            (Some((_, v)), None) => (key, ZipItem::Left(v)),
            (None, Some((_, v))) => (key, ZipItem::Right(v)),
            (None, None) => unreachable!("Iterator was built from a.keys.chain(b.keys)"),
        }
    })
}

/// Merge two iterators according to the order, like how it's done in merge sort.
pub fn merge_descending_iters<'t, T: Ord + 't>(
    iter1: impl IntoIterator<Item = &'t T> + 't,
    iter2: impl IntoIterator<Item = &'t T> + 't,
) -> impl Iterator<Item = &'t T> {
    // Reverse the comparators so we can make use of the fact that Some(X) > None.
    let mut iter1 = iter1.into_iter().peekable();
    let mut iter2 = iter2.into_iter().peekable();
    std::iter::from_fn(move || {
        if iter1.peek() > iter2.peek() {
            iter1.next()
        } else {
            iter2.next()
        }
    })
}

pub enum Either<L, R> {
    Left(L),
    Right(R),
}

impl<L, R> Either<L, R> {
    pub fn into_iter<I>(self) -> impl Iterator<Item = I>
    where
        L: Iterator<Item = I>,
        R: Iterator<Item = I>,
    {
        let (l_iter, r_iter) = match self {
            Either::Left(l) => (Some(l), None),
            Either::Right(r) => (None, Some(r)),
        };
        l_iter
            .into_iter()
            .flatten()
            .chain(r_iter.into_iter().flatten())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet, HashMap};

    use crate::utils::{zip_btree_map, ZipItem};

    use super::{merge_descending_iters, zip_hash_map};

    #[test]
    fn test_zip_hash_map() {
        let english = HashMap::<_, _>::from_iter([(0, "zero"), (1, "one"), (2, "two")]);
        let spanish = HashMap::<_, _>::from_iter([(1, "uno"), (2, "dos"), (3, "tres")]);
        let zipped: HashMap<_, _> = zip_hash_map(&english, &spanish).collect();
        assert_eq!(
            zipped,
            HashMap::from_iter([
                (&0, ZipItem::Left(&"zero")),
                (&1, ZipItem::Both(&"one", &"uno")),
                (&2, ZipItem::Both(&"two", &"dos")),
                (&3, ZipItem::Right(&"tres")),
            ])
        )
    }

    #[test]
    fn test_zip_btree_map() {
        let english = BTreeMap::from_iter([(0, "zero"), (1, "one"), (2, "two")]);
        let spanish = BTreeMap::from_iter([(1, "uno"), (2, "dos"), (3, "tres")]);
        let zipped: BTreeMap<_, _> = zip_btree_map(&english, &spanish).collect();
        assert_eq!(
            zipped,
            BTreeMap::from_iter([
                (&0, ZipItem::Left(&"zero")),
                (&1, ZipItem::Both(&"one", &"uno")),
                (&2, ZipItem::Both(&"two", &"dos")),
                (&3, ZipItem::Right(&"tres")),
            ])
        )
    }

    /// The implementations for [`zip_hash_map`] and [`zip_btree_map`] should be the same, except
    /// for iteration order.
    #[derive_fuzztest::proptest]
    fn zip_results_equal(map1: Vec<(u8, u8)>, map2: Vec<(u8, u8)>) {
        let hashmap1 = HashMap::<_, _>::from_iter(map1.clone());
        let hashmap2 = HashMap::<_, _>::from_iter(map2.clone());
        let btreemap1 = BTreeMap::from_iter(map1.clone());
        let btreemap2 = BTreeMap::from_iter(map2.clone());
        let zipped_hashmap = zip_hash_map(&hashmap1, &hashmap2);
        let zipped_btreemap = zip_btree_map(&btreemap1, &btreemap2);

        assert_eq!(
            zipped_hashmap.collect::<BTreeSet<_>>(),
            zipped_btreemap.collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn test_merge_iters() {
        let iter1 = [5, 4, 3, 2, 1];
        let iter2 = [8, 6, 4, 2];
        let merged = merge_descending_iters(&iter1, &iter2).collect::<Vec<_>>();
        assert_eq!([&8, &6, &5, &4, &4, &3, &2, &2, &1].as_slice(), &merged);
    }
}
