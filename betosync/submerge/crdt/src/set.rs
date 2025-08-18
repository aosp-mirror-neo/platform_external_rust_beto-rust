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

//! A set CRDT that resolves concurrent modifications by comparing the generation number (a.k.a. the
//! causal length).
//!
//! See [`Set`].

use std::{
    collections::{hash_map::RandomState, HashMap, HashSet},
    fmt::Debug,
    hash::{BuildHasher, Hash},
};

use crate::{
    delta::{AsDeltaMut, AsDeltaRef},
    utils::{zip_hash_map, ZipItem},
    ContentEq, CrdtState,
};
use arbitrary::{Arbitrary, Unstructured};
use derive_where::derive_where;
use serde::{Deserialize, Serialize};

/// The metadata associated with an element in the set. This is an alternative encoding to the
/// "causal-length" described in the paper. The paper uses odd numbers to represent `removed =
/// false` and even numbers to represent `removed = true`, whereas this uses a separate boolean to
/// be more explicit.
///
/// Translation from this struct to causal-length can be done by `causal-length = generation * 2 +
/// if removed { 1 } else { 0 } - 1`.
#[derive(Debug, Clone, Copy, Arbitrary, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct ElementState {
    /// The generation number of this element. During merge, the element with higher generation
    /// wins.
    generation: u32,
    /// Whether the element is removed. The element itself needs to be kept as CRDT metadata (a.k.a.
    /// tombstone) to ensure convergence. During merge, `removed = true` wins for elements with the
    /// same generation number.
    removed: bool,
}

impl Default for ElementState {
    fn default() -> Self {
        Self {
            generation: 0,
            removed: true,
        }
    }
}

/// A Causal Length Set, which is a set CRDT that resolves concurrent modifications by comparing
/// the generation number (a.k.a. the causal length).
///
/// The generation number is incremented when an entry goes from removed state to added state. That
/// means the more a node has observed an entry being (re-)added, the more likely its modification
/// will win during a merge. Adding a value that already exists or removing a value that is not in
/// the set are no-ops.
///
/// Reference: [Causal-Length Set](https://dl.acm.org/doi/abs/10.1145/3380787.3393678)
///
/// Values of this set must implement [`Eq`]. In addition to the reflexive, symmetric, and
/// transitive properties required by `Eq`, the value should also make sure that any non-trivial
/// fields (i.e. fields sent over the wire) participate in the `Eq` comparison. Convergence of
/// fields not participating in the `Eq` comparison is not guaranteed.
///
/// Garbage collection is not supported, meaning removed elements (a.k.a. "tombstones") remain in
/// the set as metadata.
///
/// # Implementations
/// * See [`SetRead`] for methods on read-only references of [`Set`].
/// * See [`SetWrite`] for methods on mutable references of [`Set`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive_where(Default; S)]
#[derive_where(PartialEq, Eq; E: Eq + Hash, S: BuildHasher)]
#[serde(transparent)]
pub struct Set<E, S = RandomState> {
    #[serde(bound(
        serialize = "E: Serialize",
        deserialize = "E: Deserialize<'de> + Eq + Hash, S: BuildHasher + Default"
    ))]
    elements: HashMap<E, ElementState, S>,
}

// Manual implementation of Arbitrary that always have no base, and is not bound on `S: Arbitrary`.
impl<'a, E, S> Arbitrary<'a> for Set<E, S>
where
    E: Hash + Eq + Arbitrary<'a>,
    S: BuildHasher + Default,
{
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self {
            elements: u.arbitrary_iter()?.collect::<Result<_, _>>()?,
        })
    }
}

/// Cannot add new element to the set because the generation number space is exhausted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationExhausted;

/// A private helper function, intended for use by [`SetRead`] to get the element state taking
/// both the base and the delta into account.
fn get_entry<E, S>(
    base: Option<&Set<E, S>>,
    delta: Option<&Set<E, S>>,
    value: &E,
) -> Option<ElementState>
where
    E: Hash + Eq,
    S: BuildHasher,
{
    let original = base.and_then(|b| b.elements.get(value));
    let delta = delta.and_then(|delta| delta.elements.get(value));
    match (original, delta) {
        (None, None) => None,
        (Some(s), None) | (None, Some(s)) => Some(*s),
        (Some(o), Some(d)) => Some(*o.max(d)),
    }
}

fn iter_entries<'a, E, S>(
    base: Option<&'a Set<E, S>>,
    delta: Option<&'a Set<E, S>>,
) -> impl Iterator<Item = (&'a E, &'a ElementState)>
where
    S: 'static,
    E: Hash + Eq,
    S: BuildHasher + Default,
{
    enum HeterogeneousIter<I1, I2, I3> {
        Empty,
        Iter1(I1),
        Iter2(I2),
        Iter3(I3),
    }
    impl<I1, I2, I3> Iterator for HeterogeneousIter<I1, I2, I3>
    where
        I1: Iterator,
        I2: Iterator<Item = I1::Item>,
        I3: Iterator<Item = I1::Item>,
    {
        type Item = I1::Item;

        fn next(&mut self) -> Option<Self::Item> {
            match self {
                HeterogeneousIter::Empty => None,
                HeterogeneousIter::Iter1(i1) => i1.next(),
                HeterogeneousIter::Iter2(i2) => i2.next(),
                HeterogeneousIter::Iter3(i3) => i3.next(),
            }
        }
    }

    match (base, delta) {
        (None, None) => HeterogeneousIter::Empty,
        (None, Some(delta)) => HeterogeneousIter::Iter1(delta.elements.iter()),
        (Some(base), None) => HeterogeneousIter::Iter2(base.elements.iter()),
        (Some(base), Some(delta)) => {
            HeterogeneousIter::Iter3(zip_hash_map(&base.elements, &delta.elements).map(
                |(key, item)| -> (&E, &ElementState) {
                    match item {
                        ZipItem::Left(elem) | ZipItem::Right(elem) => (key, elem),
                        ZipItem::Both(left, right) => (key, left.max(right)),
                    }
                },
            ))
        }
    }
}

/// Read-only operations for this Set CRDT.
///
/// ## See also
/// * See [`Set`] for a description of this set type.
/// * See [`SetWrite`] for mutating operations on this set type.
pub trait SetRead<E, S>: AsDeltaRef<Set<E, S>> {
    /// Get the set of entries currently in the map.
    fn entries(&self) -> HashSet<&E>
    where
        S: 'static,
        E: Hash + Eq,
        S: BuildHasher + Default,
    {
        iter_entries(self.base(), self.delta())
            .filter(|(_, element_state)| !element_state.removed)
            .map(|(element, _)| element)
            .collect()
    }

    /// Returns `true` if the set contains `value`.
    fn contains(&self, value: &E) -> bool
    where
        E: Hash + Eq,
        S: BuildHasher,
    {
        get_entry(self.base(), self.delta(), value)
            .map(|state| !state.removed)
            .unwrap_or(false)
    }

    /// Copies the data without the associated metadata and returns the plain type.
    ///
    /// See also: [`crate::HasPlainRepresentation`].
    fn to_plain(&self) -> HashSet<E, S>
    where
        E: Hash + Eq + Clone,
        S: BuildHasher + Default + 'static,
    {
        self.entries().into_iter().cloned().collect()
    }
}

/// Mutating operations for this Set CRDT.
///
/// ## See also
/// * See [`Set`] for a description of this set type.
/// * See [`SetRead`] for read-only operations of this set type.
pub trait SetWrite<E, S>: SetRead<E, S> + AsDeltaMut<Set<E, S>> {
    /// Add an element into the set.
    ///
    /// # Returns
    /// True if the set has changed. False if the value is already in the set.
    fn add(&mut self, value: E) -> Result<bool, GenerationExhausted>
    where
        E: Hash + Eq,
        S: BuildHasher,
    {
        let generation = match get_entry(self.base(), self.delta(), &value) {
            Some(entry) => {
                if entry.removed {
                    entry.generation.checked_add(1).ok_or(GenerationExhausted)?
                } else {
                    return Ok(false);
                }
            }
            None => 1,
        };
        let _ = self.delta_mut().elements.insert(
            value,
            ElementState {
                generation,
                removed: false,
            },
        );
        Ok(true)
    }

    /// Removes an element from the set.
    ///
    /// Note that the element will be kept as metadata, but will be marked as removed such that it
    /// won't show up in [`SetRead::entries`].
    ///
    /// # Returns
    /// True if the value is removed. False if the value was not in the set to begin with.
    fn remove(&mut self, value: E) -> bool
    where
        E: Hash + Eq,
        S: BuildHasher,
    {
        if let Some(v) = get_entry(self.base(), self.delta(), &value) {
            if !v.removed {
                let _ = self.delta_mut().elements.insert(
                    value,
                    ElementState {
                        generation: v.generation,
                        removed: true,
                    },
                );
                return true;
            }
        }
        false
    }

    /// Adds the given `value` to the set, and bumps the generation number by `increment`.
    ///
    /// Intended for testing only, so tests can artificially create a set with a large generation
    /// number, and test the behavior when the generation number is exhausted. Unlike
    /// [`add`][SetWrite::add], this will increment the generation number even if `value` is already
    /// in the set.
    #[cfg(any(test, feature = "testing"))]
    fn bump_generation_number_for_testing(
        &mut self,
        value: E,
        increment: u32,
    ) -> Result<(), GenerationExhausted>
    where
        E: Hash + Eq,
        S: BuildHasher,
    {
        let generation = match get_entry(self.base(), self.delta(), &value) {
            Some(entry) => entry
                .generation
                .checked_add(increment)
                .ok_or(GenerationExhausted)?,
            None => increment,
        };
        let _ = self.delta_mut().elements.insert(
            value,
            ElementState {
                generation,
                removed: false,
            },
        );
        Ok(())
    }

    /// Apply the changes in the plain representation into this CRDT. This assumes that all changes
    /// in the plain representation were made by the calling node, updating the associated CRDT
    /// metadata in the process.
    ///
    /// After applying the changes, [`to_plain`][SetRead::to_plain] should return the same
    /// value as `plain`.
    ///
    /// # Returns
    ///
    /// `true` if applying the change results in an update in the CRDT state, or false if Returns
    /// true if applying the change results in an update in the CRDT state, or false if `plain` is
    /// the same as [`to_plain`][SetRead::to_plain] to begin with. This can be used to avoid
    /// sending unnecessary update messages over the network, or writing changes to disk
    /// unnecessarily.
    fn apply_changes(&mut self, plain: HashSet<E, S>) -> Result<bool, GenerationExhausted>
    where
        E: Hash + Eq + Clone,
        S: BuildHasher + Default + 'static,
    {
        let mut changed = false;
        let to_be_removed: Vec<_> = self
            .entries()
            .into_iter()
            .filter(|e| !plain.contains(e))
            .cloned()
            .collect();
        for e in to_be_removed {
            let _ = self.remove(e);
            changed = true;
        }
        for e in plain {
            changed = self.add(e)? || changed;
        }
        Ok(changed)
    }
}

impl<E, S> CrdtState for Set<E, S>
where
    E: Hash + Eq + Clone,
    S: BuildHasher + Default,
{
    fn merge(a: &Self, b: &Self) -> Self {
        Self {
            elements: {
                zip_hash_map(&a.elements, &b.elements)
                    .map(|(key, item)| match item {
                        ZipItem::Left(v) | ZipItem::Right(v) => (key.clone(), *v),
                        ZipItem::Both(l, r) => (key.clone(), *l.max(r)),
                    })
                    .collect()
            },
        }
    }
}

impl<E, S> ContentEq for Set<E, S>
where
    E: Hash + Eq,
    S: BuildHasher,
{
    fn content_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl<E, S, T> SetRead<E, S> for T where T: AsDeltaRef<Set<E, S>> {}
impl<E, S, T> SetWrite<E, S> for T where T: AsDeltaMut<Set<E, S>> {}

/// Checker to help implement invariant tests over arbitrary operations on a set.
///
/// Requires the feature _`checker`_.
#[cfg(any(test, feature = "checker"))]
pub mod checker {
    use std::{collections::HashSet, fmt::Debug, hash::Hash};

    use arbitrary::Arbitrary;

    use crate::{
        checker::{
            simulation::{Operation, SimulationContext},
            test_fakes::TriState,
            utils::DeterministicHasher,
        },
        delta::DeltaMut,
        set::SetRead,
    };

    use super::{Set, SetWrite};

    /// Mutation operations on a [`SetWrite`].
    ///
    /// Can be used with [`Arbitrary`] to generate arbitrary operations to be applied on the set.
    #[derive(Debug, Clone, Arbitrary)]
    pub enum SetOp<E: Eq + Hash> {
        /// Represents the [`SetWrite::add`] operation.
        Add {
            /// The value passed to [`SetWrite::add`].
            value: E,
        },
        /// Represents the [`SetWrite::remove`] operation.
        Remove {
            /// The value passed to [`SetWrite::remove`].
            value: E,
        },
        /// Represents the [`SetWrite::apply_changes`] operation.
        ApplyChanges {
            /// The plain representation of the set passed to [`SetWrite::apply_changes`].
            value_set: HashSet<E, DeterministicHasher>,
        },
    }

    impl<V> Operation<Set<V, DeterministicHasher>, TriState> for SetOp<V>
    where
        V: Clone + Eq + Hash + Debug,
    {
        fn apply(
            self,
            mut state: DeltaMut<Set<V, DeterministicHasher>>,
            _: &SimulationContext<TriState>,
        ) {
            match self {
                SetOp::Add { value } => {
                    if state.add(value.clone()).is_ok() {
                        assert!(state.contains(&value));
                    }
                }
                SetOp::Remove { value } => {
                    let _ = state.remove(value.clone());
                    assert!(!state.contains(&value));
                }
                SetOp::ApplyChanges { value_set } => {
                    if state.apply_changes(value_set.clone()).is_ok() {
                        assert_eq!(HashSet::from_iter(&value_set), state.entries())
                    }
                }
            }
        }
    }
}

#[cfg(feature = "proto")]
mod proto {
    use submerge_internal_proto::{FromProto, FromProtoError, NodeMapping, ToProto};

    use super::{ElementState, Set};

    impl FromProto for Set<Vec<u8>> {
        type Proto = submerge_internal_proto::protos::submerge::SubmergeSet;

        fn from_proto(proto: &Self::Proto, _node_ids: &[String]) -> Result<Self, FromProtoError> {
            Ok(Self {
                elements: proto
                    .elements
                    .iter()
                    .map(|elem| {
                        (
                            elem.value.clone(),
                            ElementState {
                                generation: elem.generation,
                                removed: elem.removed,
                            },
                        )
                    })
                    .collect(),
            })
        }
    }

    impl ToProto for Set<Vec<u8>> {
        type Proto = submerge_internal_proto::protos::submerge::SubmergeSet;

        fn to_proto(&self, _node_ids: &mut NodeMapping<String>) -> Self::Proto {
            submerge_internal_proto::protos::submerge::SubmergeSet {
                elements: self
                    .elements
                    .iter()
                    .map(|(value, set_element)| {
                        submerge_internal_proto::protos::submerge::submerge_set::SetElement {
                            value: value.clone(),
                            generation: set_element.generation,
                            removed: set_element.removed,
                            ..Default::default()
                        }
                    })
                    .collect(),
                ..Default::default()
            }
        }
    }

    #[cfg(test)]
    #[derive_fuzztest::proptest]
    fn set_roundtrip(set: Set<Vec<u8>>) {
        let mut node_ids = NodeMapping::default();
        assert_eq!(
            Set::from_proto(&set.to_proto(&mut node_ids), &node_ids.into_vec()).unwrap(),
            set
        );
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::{
        checker::{test_fakes::TriState, utils::DeterministicHasher},
        set::GenerationExhausted,
        CrdtState,
    };

    use super::{Set, SetRead, SetWrite};

    type TestSet = Set<u8, DeterministicHasher>;

    #[test]
    fn test_add() {
        let mut set: TestSet = Set::default();
        let _ = set.add(1).unwrap();
        let _ = set.add(2).unwrap();
        let _ = set.add(3).unwrap();

        assert_eq!(HashSet::from_iter(&[1, 2, 3]), set.entries());
    }

    #[test]
    fn test_remove() {
        let mut set: TestSet = Set::default();
        let _ = set.add(1).unwrap();
        let _ = set.add(2).unwrap();
        let _ = set.remove(3);
        let _ = set.remove(1);

        assert_eq!(HashSet::from_iter(&[2]), set.entries());
    }

    #[test]
    fn test_generation_exhausted() {
        let mut set = Set::<TriState>::default();
        set.bump_generation_number_for_testing(TriState::A, u32::MAX)
            .unwrap();
        let _ = set.add(TriState::A).unwrap(); // This is a no-op since 1 is already in the set
        let _ = set.add(TriState::B).unwrap();
        let _ = set.remove(TriState::A);
        assert_eq!(GenerationExhausted, set.add(TriState::A).unwrap_err());
        let _ = set.add(TriState::C).unwrap(); // Adding other values should still work

        assert_eq!(
            HashSet::from_iter(&[TriState::B, TriState::C]),
            set.entries()
        );
    }

    #[test]
    fn test_merge() {
        let mut set1: TestSet = Set::default();
        let _ = set1.add(1).unwrap();
        let _ = set1.add(2).unwrap();
        let _ = set1.add(3).unwrap();

        let mut set2: TestSet = Set::default();
        let _ = set2.add(2).unwrap();
        let _ = set2.add(3).unwrap();
        let _ = set2.add(4).unwrap();

        let merged = CrdtState::merge(&set1, &set2);

        assert_eq!(HashSet::from_iter(&[1, 2, 3, 4]), merged.entries());
    }

    #[test]
    fn test_remove_remote() {
        let mut set1: TestSet = Set::default();
        let _ = set1.add(1).unwrap();
        let _ = set1.add(2).unwrap();
        let _ = set1.add(3).unwrap();

        let set2: TestSet = Set::default();

        let mut set2 = CrdtState::merge(&set1, &set2);
        let _ = set2.remove(2);

        assert_eq!(HashSet::from_iter(&[1, 3]), set2.entries());
    }
}
