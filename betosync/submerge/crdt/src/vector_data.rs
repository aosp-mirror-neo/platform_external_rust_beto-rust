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

//! A vector data with one entry per node, where each node can only modify their own entry.
//!
//! See [`VectorData`] for details.

use std::{cmp::max_by_key, collections::BTreeMap};

use crate::{
    delta::{AsDeltaMut, AsDeltaRef},
    utils::{zip_btree_map, Either, ZipItem},
    ContentEq, CrdtState, UpdateContext,
};
use arbitrary::Arbitrary;
use derive_where::derive_where;
use serde::{Deserialize, Serialize};

/// An entry in the vector data.
///
/// When `data` is modified, the `version` must also be incremented.
#[derive(Debug, Clone, Arbitrary, PartialEq, Eq, Serialize, Deserialize)]
struct VecEntry<V> {
    version: u32,
    data: Option<V>,
}

/// A vector of data with one entry per node, where each node can only modify their own entry.
///
/// The name draws similarity to how "vector clock" is a vector of logical clocks and how "version
/// vector" is a vector of version numbers.
///
/// The data is tagged with a monotonically increasing version number, which is guaranteed to be
/// unique because there is only a single writer.
///
/// When this type is merged, the vector is merged entry-by-entry, taking the entry with the higher
/// version number. Since the local version number is always incremented on data change, an equal
/// version number must imply that the data `V` is also equal.
///
/// # Implementations
/// * See [`VectorDataRead`] for methods on read-only references of [`VectorData`].
/// * See [`VectorDataWrite`] for methods on mutable references of [`VectorData`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Arbitrary)]
#[derive_where(Default)]
pub struct VectorData<N: Ord, V> {
    elements: BTreeMap<N, VecEntry<V>>,
}

/// Error applying a change to [`VectorData`] because the version number will overflow.
#[derive(Debug)]
pub struct VersionOverflow;

/// Read-only operations for this vector data CRDT.
///
/// ## See also
/// * See [`VectorData`] for a description of this vector data type.
/// * See [`VectorDataWrite`] for mutating operations on this vector data type.
pub trait VectorDataRead<N: Ord, V>: AsDeltaRef<VectorData<N, V>> {
    /// Get all values from the vector.
    ///
    /// # Returns
    /// A map with values from every node. Clients can consider using functions like `fold` to
    /// compute an aggregate value from this.
    fn entries(&self) -> BTreeMap<&N, &V>
    where
        N: 'static,
        V: 'static,
    {
        let iter_opt = match (self.base(), self.delta()) {
            (None, None) => None,
            (None, Some(v)) | (Some(v), None) => Some(Either::Left(v.elements.iter())),
            (Some(base), Some(delta)) => {
                let vec = zip_btree_map(&base.elements, &delta.elements)
                    .map(|(k, item)| match item {
                        ZipItem::Left(l) => (k, l),
                        ZipItem::Right(r) => (k, r),
                        ZipItem::Both(_, r) => (k, r),
                    })
                    .collect::<Vec<_>>()
                    .into_iter();
                Some(Either::Right(vec))
            }
        };
        iter_opt
            .into_iter()
            .flat_map(|e| e.into_iter())
            .filter_map(|(k, v)| Some((k, v.data.as_ref()?)))
            .collect()
    }

    /// Copies the data without the associated metadata and returns the plain type.
    ///
    /// See also: [`crate::HasPlainRepresentation`].
    fn to_plain(&self) -> BTreeMap<N, V>
    where
        N: Clone + 'static,
        V: Clone + 'static,
    {
        self.entries()
            .into_iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

/// Mutating operations for this vector data CRDT.
///
/// ## See also
/// * See [`VectorData`] for a description of this vector data type.
/// * See [`VectorDataRead`] for read-only operations of this vector data type.
pub trait VectorDataWrite<N: Ord, V>: VectorDataRead<N, V> + AsDeltaMut<VectorData<N, V>> {
    /// Set the value to `value` at the given `id`.
    ///
    /// # Returns
    /// `true` if the value was inserted or updated, or `false` if `value` is already equal to the
    /// entry in the vector.
    fn set(&mut self, id: N, value: Option<V>) -> Result<bool, VersionOverflow>
    where
        N: 'static,
        V: PartialEq + 'static,
    {
        let current_delta_entry = self.delta().and_then(|delta| delta.elements.get(&id));
        let current_base_entry = self.base().and_then(|b| b.elements.get(&id));
        let current_entry = max_by_key(current_base_entry, current_delta_entry, |entry_opt| {
            entry_opt.map(|e| e.version)
        });
        let new_version = match current_entry {
            Some(entry) => {
                if value == entry.data {
                    return Ok(false);
                }
                entry.version.checked_add(1).ok_or(VersionOverflow)?
            }
            None => 1,
        };
        let _ = self.delta_mut().elements.insert(
            id,
            VecEntry {
                version: new_version,
                data: value,
            },
        );
        Ok(true)
    }

    /// Sets the value to `None`, and bumps the version number by `increment`.
    ///
    /// Intended for testing only, so tests can artificially create a set with a large version
    /// number, and test the behavior when the version number overflows.
    #[cfg(any(test, feature = "testing"))]
    fn bump_version_number_for_testing(
        &mut self,
        id: N,
        increment: u32,
    ) -> Result<(), VersionOverflow> {
        let current_delta_entry = self.delta().and_then(|delta| delta.elements.get(&id));
        let current_base_entry = self.base().and_then(|b| b.elements.get(&id));
        let current_entry = max_by_key(current_base_entry, current_delta_entry, |entry_opt| {
            entry_opt.map(|e| e.version)
        });
        let new_version = match current_entry {
            Some(entry) => entry
                .version
                .checked_add(increment)
                .ok_or(VersionOverflow)?,
            None => increment,
        };
        let _ = self.delta_mut().elements.insert(
            id,
            VecEntry {
                version: new_version,
                data: None,
            },
        );
        Ok(())
    }

    /// Apply the changes in the plain representation into this CRDT. This assumes that all changes
    /// in the plain representation were made by the calling node, updating the associated CRDT
    /// metadata in the process.
    ///
    /// After applying the changes, [`to_plain`][VectorDataRead::to_plain] should return the same
    /// value as `plain`.
    ///
    /// # Returns
    ///
    /// `true` if applying the change results in an update in the CRDT state, or false if Returns
    /// true if applying the change results in an update in the CRDT state, or false if `plain` is
    /// the same as [`to_plain`][VectorDataRead::to_plain] to begin with. This can be used to avoid
    /// sending unnecessary update messages over the network, or writing changes to disk
    /// unnecessarily.
    fn apply_changes(
        &mut self,
        ctx: &impl UpdateContext<N>,
        mut plain: BTreeMap<N, V>,
    ) -> Result<bool, VectorDataError>
    where
        N: Clone + 'static,
        V: Clone + Eq + 'static,
    {
        let updater = ctx.updater();
        let entries: BTreeMap<_, _> = self.entries().into_iter().collect();
        // Validate the plain representation, making sure only the entry for the updater is changed.
        for (key, item) in zip_btree_map::<N, N, &N, V, &V>(&plain, &entries) {
            match item {
                ZipItem::Left(_) | ZipItem::Right(_) => {
                    if key != updater {
                        return Err(VectorDataError::MismatchedNode);
                    }
                }
                ZipItem::Both(plain, crdt) => {
                    if key != updater && plain != *crdt {
                        return Err(VectorDataError::MismatchedNode);
                    }
                }
            }
        }

        let plain_entry = plain.remove(updater);
        if self.entries().remove(updater) == plain_entry.as_ref() {
            return Ok(false);
        }
        Ok(self.set(updater.clone(), plain_entry)?)
    }
}

impl<N, V> CrdtState for VectorData<N, V>
where
    N: Ord + Clone,
    V: Clone + Eq,
{
    fn merge(a: &Self, b: &Self) -> Result<Self, crate::MergeError> {
        Ok(Self {
            elements: zip_btree_map(&a.elements, &b.elements)
                .map(|(key, item)| {
                    let entry = match item {
                        ZipItem::Left(value) => value.clone(),
                        ZipItem::Right(value) => value.clone(),
                        ZipItem::Both(value_a, value_b) => {
                            if value_a.version == value_b.version && value_a.data != value_b.data {
                                return Err(crate::MergeError::InconsistentValue);
                            }
                            max_by_key(value_a, value_b, |v| v.version).clone()
                        }
                    };
                    Ok((key.clone(), entry))
                })
                .collect::<Result<_, _>>()?,
        })
    }

    #[cfg(any(test, feature = "checker"))]
    fn is_valid_collection<'a>(collection: impl IntoIterator<Item = &'a Self>) -> bool
    where
        Self: 'a,
    {
        use crate::checker::utils::IterExt as _;

        // Convergence requirement: Entries from the same node that have the same version number
        // must contain the same data. This is enforced for local modifications by always
        // incrementing the version number when new data is `set`. Concurrent modifications are not
        // allowed because each node can only write to its own entry in the vector.
        collection
            .into_iter()
            .iter_pairs_unordered()
            .all(|(s1, s2)| {
                s1.elements.keys().chain(s2.elements.keys()).all(|k| {
                    match (s1.elements.get(k), s2.elements.get(k)) {
                        (Some(v1), Some(v2)) => v1 == v2 || v1.version != v2.version,
                        _ => true,
                    }
                })
            })
    }
}

impl<N, V> ContentEq for VectorData<N, V>
where
    N: Ord,
    V: PartialEq,
{
    fn content_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl<N: Ord, V, T: AsDeltaRef<VectorData<N, V>>> VectorDataRead<N, V> for T {}
impl<N: Ord, V, T: AsDeltaMut<VectorData<N, V>>> VectorDataWrite<N, V> for T {}

/// Error returned from [`VectorDataWrite::apply_changes`].
#[derive(Debug, PartialEq, Eq)]
pub enum VectorDataError {
    /// Changes cannot be applied because data corresponding to a node other than the local node has
    /// been modified.
    MismatchedNode,
    /// Error applying a change to [`VectorData`] because the version number will overflow.
    VersionOverflow,
}

impl From<VersionOverflow> for VectorDataError {
    fn from(_: VersionOverflow) -> Self {
        VectorDataError::VersionOverflow
    }
}

/// Checker to help implement invariant tests over arbitrary operations on a vector data.
///
/// Requires the feature _`checker`_.
#[cfg(any(test, feature = "checker"))]
pub mod checker {
    use std::{collections::BTreeMap, fmt::Debug};

    use arbitrary::Arbitrary;

    use crate::{
        checker::simulation::{Operation, SimulationContext},
        delta::{AsDeltaRef, DeltaMut},
        vector_data::VectorDataRead,
    };

    use super::{VectorData, VectorDataWrite};

    /// Mutation operations on a [`VectorDataWrite`].
    ///
    /// Can be used with [`Arbitrary`] to generate arbitrary operations to be applied on the vector
    /// data.
    #[derive(Debug, Clone, Arbitrary)]
    #[allow(missing_docs)]
    pub enum VecDataOp<N: Ord, V> {
        Set { node: N, value: V },
        ApplyChanges { node: N, plain: BTreeMap<N, V> },
    }

    impl<N, V> Operation<VectorData<N, V>, N> for VecDataOp<N, V>
    where
        N: Debug + Ord + Clone + 'static,
        V: Debug + PartialEq + Eq + Clone + 'static,
    {
        fn apply(self, mut state: DeltaMut<VectorData<N, V>>, ctx: &SimulationContext<N>) {
            let before = state.as_ref().merge().expect("merge must not fail");
            let before_entries = before.entries();
            match self {
                VecDataOp::Set { node, value } => {
                    let result = state.set(node.clone(), Some(value.clone()));
                    let entries = state.entries();
                    if result.is_ok() {
                        assert_eq!(entries.get(&node), Some(&&value));
                    }
                    assert_non_updater_entries_unchanged(node, before_entries, entries);
                }
                VecDataOp::ApplyChanges { node, plain } => {
                    let result = state.apply_changes(&ctx.context(&node), plain.clone());
                    if result.is_ok() {
                        assert_eq!(plain, state.to_plain());
                    }
                    assert_non_updater_entries_unchanged(node, before_entries, state.entries());
                }
            }
        }
    }

    fn assert_non_updater_entries_unchanged<N, V>(
        updater: N,
        mut before_entries: BTreeMap<&N, &V>,
        mut after_entries: BTreeMap<&N, &V>,
    ) where
        N: Ord + Eq + Clone + Debug + 'static,
        V: Clone + Eq + Debug + 'static,
    {
        let _ = before_entries.remove(&updater);
        let _ = after_entries.remove(&updater);
        assert_eq!(before_entries, after_entries);
    }
}

#[cfg(feature = "proto")]
mod proto {
    use submerge_internal_proto::{FromProto, FromProtoError, NodeMapping, ToProto};

    use super::{VecEntry, VectorData};

    impl FromProto for VectorData<String, Vec<u8>> {
        type Proto = submerge_internal_proto::protos::submerge::SubmergeVectorData;

        fn from_proto(proto: &Self::Proto, node_ids: &[String]) -> Result<Self, FromProtoError> {
            Ok(Self {
                elements: proto
                    .elements
                    .iter()
                    .map(|elem| {
                        Ok((
                            node_ids
                                .get(elem.node as usize)
                                .ok_or(FromProtoError::MissingNodeId)?
                                .clone(),
                            VecEntry {
                                version: elem.version,
                                data: elem.value.clone(),
                            },
                        ))
                    })
                    .collect::<Result<_, _>>()?,
            })
        }
    }

    impl ToProto for VectorData<String, Vec<u8>> {
        type Proto = submerge_internal_proto::protos::submerge::SubmergeVectorData;

        fn to_proto(&self, node_ids: &mut NodeMapping<String>) -> Self::Proto {
            submerge_internal_proto::protos::submerge::SubmergeVectorData {
                elements: self
                    .elements
                    .iter()
                    .map(|(node, v)| {
                        submerge_internal_proto::protos::submerge::submerge_vector_data::VecElement {
                            node: node_ids.get_index(node),
                            value: v.data.clone(),
                            version: v.version,
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
    fn vector_data_roundtrip(vector_data: VectorData<String, Vec<u8>>) {
        let mut node_ids = NodeMapping::default();
        assert_eq!(
            VectorData::from_proto(&vector_data.to_proto(&mut node_ids), &node_ids.into_vec())
                .unwrap(),
            vector_data
        );
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{
        checker::test_fakes::FakeContext,
        vector_data::{VectorDataError, VectorDataRead, VectorDataWrite},
        CrdtState,
    };

    use super::VectorData;

    #[test]
    fn set_value() {
        let mut v = VectorData::<u8, u8>::default();
        let _ = v.set(0, Some(66)).unwrap();
        let _ = v.set(1, Some(88)).unwrap();
        assert_eq!(BTreeMap::from_iter([(&0, &66), (&1, &88)]), v.entries());
    }

    #[test]
    fn version_overflow() {
        let mut v = VectorData::<u8, u8>::default();
        v.bump_version_number_for_testing(0, u32::MAX).unwrap();
        let _ = v.set(0, Some(66)).unwrap_err();
        let _ = v.set(1, Some(88)).unwrap();
        assert_eq!(BTreeMap::from_iter([(&1, &88)]), v.entries());
    }

    #[test]
    fn apply_changes() {
        let mut v = VectorData::<u8, u8>::default();
        let _ = v.set(0, Some(66)).unwrap();
        let _ = v.set(1, Some(88)).unwrap();
        let mut plain = v.to_plain();
        let _ = plain.entry(0).and_modify(|e| *e = 99);

        let changed = v.apply_changes(&FakeContext::new(0, 0_u8), plain).unwrap();
        assert!(changed);

        assert_eq!(BTreeMap::from_iter([(&0, &99), (&1, &88)]), v.entries());
    }

    #[test]
    fn apply_changes_mismatched() {
        let mut v = VectorData::<u8, u8>::default();
        let _ = v.set(0, Some(66)).unwrap();
        let _ = v.set(1, Some(88)).unwrap();
        let mut plain = v.to_plain();
        let _ = plain.entry(1).and_modify(|e| *e = 99);

        let result = v.apply_changes(&FakeContext::new(0, 0_u8), plain);
        assert_eq!(VectorDataError::MismatchedNode, result.unwrap_err());
    }

    #[test]
    fn merge() {
        let mut v_parent = VectorData::<u8, u8>::default();
        let _ = v_parent.set(0, Some(66)).unwrap();

        let mut child1 = v_parent.clone();
        let _ = child1.set(1, Some(44)).unwrap();

        let _ = v_parent.set(0, Some(88)).unwrap();
        let mut child2 = v_parent.clone();
        let _ = child2.set(2, Some(22)).unwrap();

        let v_merged = VectorData::merge(&child1, &child2).unwrap();
        assert_eq!(
            BTreeMap::from_iter([(&0, &88), (&1, &44), (&2, &22)]),
            v_merged.entries()
        );
    }

    #[test]
    fn merge_undefined_behavior() {
        let mut v1 = VectorData::<u8, u8>::default();
        let _ = v1.set(1, Some(88)).unwrap();

        let mut v2 = VectorData::<u8, u8>::default();
        let _ = v2.set(1, Some(44)).unwrap();

        assert_eq!(
            VectorData::merge(&v1, &v2).unwrap_err(),
            crate::MergeError::InconsistentValue
        );
    }

    #[test]
    fn test_apply_changes() {
        let mut vector_data = VectorData::<u8, u8>::default();
        let plain = BTreeMap::from_iter([(1, 1)]);
        let _ = vector_data
            .apply_changes(&FakeContext::new(1, 0_u8), plain.clone())
            .unwrap();

        assert_eq!(plain, vector_data.to_plain());
    }

    #[test]
    fn test_apply_changes_mismatch() {
        let mut vector_data = VectorData::<u8, u8>::default();
        let plain = BTreeMap::from_iter([(1, 1)]);
        // Apply changes from Node 2, but trying to change data on Node 1
        assert_eq!(
            Err(VectorDataError::MismatchedNode),
            vector_data.apply_changes(&FakeContext::new(2, 0_u8), plain.clone())
        );
    }

    #[test]
    fn test_init_from_plain() {
        let plain = BTreeMap::from_iter([(1, 1)]);
        let mut vector_data = VectorData::default();
        let _ = vector_data
            .apply_changes(&FakeContext::new(1, 0_u8), plain.clone())
            .unwrap();
        assert_eq!(plain, vector_data.to_plain());
    }

    #[test]
    fn test_init_from_plain_empty() {
        let plain: BTreeMap<u8, u8> = BTreeMap::new();
        let mut vector_data = VectorData::default();
        let _ = vector_data
            .apply_changes(&FakeContext::new(1, 0_u8), plain.clone())
            .unwrap();
        assert_eq!(plain, vector_data.to_plain());
    }

    #[test]
    fn test_init_from_plain_mismatch() {
        let plain = BTreeMap::from_iter([(1, 1)]);
        let mut vector_data = VectorData::default();
        assert_eq!(
            Err(VectorDataError::MismatchedNode),
            vector_data.apply_changes(&FakeContext::new(2, 0_u8), plain.clone()),
        );
    }
}
