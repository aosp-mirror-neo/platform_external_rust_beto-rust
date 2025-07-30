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

//! A last-writer-wins map implementation that can contain dynamically typed values, where type info
//! modifications are merged by last-writer-wins semantics.
//!
//! See the docs in [`LwwMap`] for details.

use std::{collections::BTreeMap, fmt::Debug};

use crate::{
    delta::{AsDeltaMut, AsDeltaRef, DeltaMut, DeltaRef},
    lww_crdt_container::{LwwCrdtContainerRead, LwwCrdtContainerWrite, TryAsChild},
    utils::{zip_btree_map, Either, ZipItem},
    ApplyChanges, CrdtState, HasPlainRepresentation, ToPlain, UpdateContext,
};
use arbitrary::Arbitrary;
use derive_where::derive_where;
use distributed_time::{vector_clock::VectorClock, TimestampOverflow, TotalTimestamp};
use serde::{Deserialize, Serialize};

use crate::lww_crdt_container::{CrdtUnion, LwwCrdtContainer, LwwCrdtContainerApplyError};

/// A last-writer-wins map implementation that can contain dynamically typed values.
///
/// The values in this map can be dynamically typed CRDTs:
/// - The value type `V` contains the type information. The type can be modified using
///   [`set_value`][LwwMapWrite::set_value] or [`remove_value`][LwwMapWrite::remove_value], and is
///   merged using last-writer-wins.
/// - The value type `V` can switch between multiple child types by implementing `V: TryAsChild<D>`.
///   Child data values modified using [`get_child_mut`][LwwMapWrite::get_child_mut] are merged
///   using the child's `merge` function.
///
/// # Implementations
/// * See [`LwwMapRead`] for methods on read-only references of [`LwwMap`].
/// * See [`LwwMapWrite`] for methods on mutable references of [`LwwMap`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Arbitrary)]
#[derive_where(Default)]
#[serde(transparent)]
pub struct LwwMap<K, V, T, N>
where
    K: Ord,
    N: Ord,
{
    /// An "insert-only" map containing the values and their type information. Items must not be
    /// directly removed from or modified this map, but instead go through the APIs in
    /// [`LwwCrdtContainer`].
    elements: BTreeMap<K, LwwCrdtContainer<V, T, N>>,
}

impl<K, V, T, N> LwwMap<K, V, T, N>
where
    K: Ord,
    N: Ord,
{
    /// Calculates the delta needed to update from the given `base_version`.
    ///
    /// The `base_version` must be a version vector from a replica of the same document. The
    /// returned document can be applied using [`CrdtState::merge`] by supplying a base
    /// that is at least as new as `base_version`.
    ///
    /// Note that the version is not tracked in this crate. Users of this crate must track the
    /// versioning information externally.
    pub fn calculate_delta(&self, base_version: &VectorClock<N>) -> Self
    where
        V: CrdtUnion<N>,
        K: Clone,
        N: Clone,
        T: Clone,
    {
        LwwMap {
            elements: self
                .elements
                .iter()
                .map(|(k, v)| (k.clone(), v.calculate_delta(base_version)))
                .collect(),
        }
    }
}

/// Read-only operations for this Map CRDT.
///
/// ## See also
/// * See [`LwwMap`] for a description of the CRDT map type.
/// * See [`LwwMapWrite`] for mutating operations on CRDT maps.
pub trait LwwMapRead<K, V, T, N>: AsDeltaRef<LwwMap<K, V, T, N>>
where
    K: Ord + 'static,
    V: 'static,
    T: 'static,
    N: Ord + 'static,
{
    /// Get a reference to the container associated with `key` from this map.
    fn get_container(&self, key: &K) -> DeltaRef<'_, LwwCrdtContainer<V, T, N>> {
        let delta_container = self.delta().and_then(|delta| delta.elements.get(key));
        let base_container = self.base().and_then(|b| b.elements.get(key));
        DeltaRef {
            base: base_container,
            delta: delta_container,
        }
    }

    /// Get a reference to the child CRDT at the given key, or `None` if the value does not exist or
    /// cannot be converted to type `Child`.
    fn get_child<Child>(&self, key: &K) -> Option<DeltaRef<'_, Child>>
    where
        V: TryAsChild<Child, N> + Clone,
        T: TotalTimestamp + Clone,
    {
        self.get_container(key).into_child()
    }

    /// Get a reference to the value at the given `key`.
    fn get_value(&self, key: &K) -> Option<V::DeltaRef<'_>>
    where
        V: CrdtUnion<N> + Clone,
        T: TotalTimestamp + Clone,
    {
        self.get_container(key).into_value()
    }

    /// Get an iterator though all the keys in this map.
    fn keys(&self) -> impl Iterator<Item = &K> {
        let iter_opt = match (self.base(), self.delta()) {
            (None, None) => None,
            (None, Some(v)) | (Some(v), None) => Some(Either::Left(v.elements.keys())),
            (Some(base), Some(delta)) => {
                let iter = zip_btree_map(&base.elements, &delta.elements).map(|(key, _)| key);
                Some(Either::Right(iter))
            }
        };
        iter_opt.into_iter().flat_map(|e| e.into_iter())
    }

    /// Copies the data without the associated metadata and returns the plain type.
    ///
    /// See also: [`crate::HasPlainRepresentation`].
    fn to_plain(&self) -> BTreeMap<K, V::Plain>
    where
        K: Clone,
        V: CrdtUnion<N> + HasPlainRepresentation<N> + Clone,
        for<'d> V::DeltaRef<'d>: ToPlain<Plain = V::Plain>,
        for<'d> V::DeltaMut<'d>: ApplyChanges<N, Plain = V::Plain, Error = V::Error>,
        T: TotalTimestamp<NodeId = N> + Clone,
    {
        self.keys()
            .filter_map(|k| {
                let container = self.get_container(k);
                Some((k.clone(), container.to_plain()?))
            })
            .collect()
    }

    /// Returns the combined least upper bound of all the subtree_version of all of the items in
    /// this map.
    #[cfg(feature = "checker")]
    fn subtree_version_least_upper_bound(&self) -> VectorClock<N>
    where
        N: Clone,
    {
        use distributed_time::DistributedClock;

        let mut version = VectorClock::default();
        for key in self.keys() {
            let container = self.get_container(key);
            if let Some(delta) = container.delta() {
                version.least_upper_bound_in_place(delta.subtree_version());
            }
            if let Some(base) = container.base() {
                version.least_upper_bound_in_place(base.subtree_version());
            }
        }
        version
    }
}

/// Mutating operations for this Map CRDT.
///
/// ## See also
/// * See [`LwwMap`] for a description of the CRDT map type.
/// * See [`LwwMapRead`] for read-only operations on CRDT maps.
pub trait LwwMapWrite<K, V, T, N>: LwwMapRead<K, V, T, N> + AsDeltaMut<LwwMap<K, V, T, N>>
where
    K: Ord + 'static,
    V: 'static,
    T: 'static,
    N: Ord + Clone + 'static,
{
    /// Get a mutable reference to the container associated with `key` from this map.
    fn get_container_mut(&mut self, key: K) -> DeltaMut<'_, LwwCrdtContainer<V, T, N>> {
        let self_mut = self.as_mut();
        let base = self_mut.base.and_then(|base| base.elements.get(&key));
        let delta = self_mut.delta.elements.entry(key).or_default();
        DeltaMut { base, delta }
    }

    /// Get a mutable reference to the value at the given `key`.
    ///
    /// This is a _subtree version updating operation_ (See
    /// [`subtree_version`](LwwCrdtContainer::subtree_version)).
    fn get_value_mut(
        &mut self,
        ctx: &impl UpdateContext<N>,
        key: K,
    ) -> Result<Option<V::DeltaMut<'_>>, TimestampOverflow>
    where
        V: CrdtUnion<N>,
        T: Clone + Ord,
    {
        self.get_container_mut(key).into_value_mut(ctx)
    }

    /// Get a mutable reference to the child CRDT at the given key, or `None` if the value does not
    /// exist or cannot be converted to type `Child`.
    ///
    /// This is a _subtree version updating operation_ (See
    /// [`subtree_version`](LwwCrdtContainer::subtree_version)).
    fn get_child_mut<Child>(
        &mut self,
        ctx: &impl UpdateContext<N>,
        key: K,
    ) -> Result<Option<DeltaMut<'_, Child>>, TimestampOverflow>
    where
        Child: Default,
        V: TryAsChild<Child, N> + Clone,
        T: TotalTimestamp + Clone,
    {
        self.get_container_mut(key).into_child_mut(ctx)
    }

    /// Get a mutable reference to the child CRDT at the given key, or set the value to
    /// `D::default()` if it does not already exist.
    ///
    /// This is a _subtree version updating operation_ (See
    /// [`subtree_version`](LwwCrdtContainer::subtree_version)).
    // TODO: This is probably a bad way to do schema management, should consider removing it.
    fn get_child_or_default<Child>(
        &mut self,
        ctx: &impl UpdateContext<N>,
        key: K,
    ) -> Result<Option<DeltaMut<'_, Child>>, TimestampOverflow>
    where
        Child: Default,
        V: TryAsChild<Child, N> + Clone,
        T: TotalTimestamp<NodeId = N> + Clone,
    {
        self.get_container_mut(key).into_child_or_default(ctx)
    }

    /// Set the value associated with `key` in this map.
    ///
    /// Setting the value will increment the timestamp associated with this value, and will
    /// overwrite other concurrent changes made via [`LwwMapWrite::get_child_mut`].
    fn set_value(
        &mut self,
        ctx: &impl UpdateContext<N>,
        key: K,
        value: V,
    ) -> Result<(), TimestampOverflow>
    where
        V: CrdtUnion<N> + Clone,
        T: TotalTimestamp<NodeId = N> + Clone,
    {
        self.get_container_mut(key).set_value(ctx, value)
    }

    /// Remove the child CRDT at the given key from this map.
    fn remove_value(&mut self, ctx: &impl UpdateContext<N>, key: K) -> Result<(), TimestampOverflow>
    where
        V: CrdtUnion<N> + Clone,
        T: TotalTimestamp<NodeId = N> + Clone,
    {
        self.get_container_mut(key).remove_value(ctx)
    }

    /// Apply the changes in the plain representation into this CRDT. This assumes that all changes
    /// in the plain representation were made by the calling node, updating the associated CRDT
    /// metadata in the process.
    ///
    /// After applying the changes, [`to_plain`][LwwMapRead::to_plain] should return the same value
    /// as `plain`.
    ///
    /// # Returns
    ///
    /// `true` if applying the change results in an update in the CRDT state, or false if Returns
    /// true if applying the change results in an update in the CRDT state, or false if `plain` is
    /// the same as [`to_plain`][LwwMapRead::to_plain] to begin with. This can be used to avoid
    /// sending unnecessary update messages over the network, or writing changes to disk
    /// unnecessarily.
    fn apply_changes(
        &mut self,
        ctx: &impl UpdateContext<N>,
        plain: BTreeMap<K, V::Plain>,
    ) -> Result<bool, LwwCrdtContainerApplyError<V::Error>>
    where
        K: Ord + Clone,
        V: CrdtUnion<N> + HasPlainRepresentation<N> + Clone,
        for<'d> V::DeltaRef<'d>: ToPlain<Plain = V::Plain>,
        for<'d> V::DeltaMut<'d>: ApplyChanges<N, Plain = V::Plain, Error = V::Error>,
        T: TotalTimestamp<NodeId = N> + Clone,
    {
        let mut changed = false;
        let keys: Vec<_> = self.keys().cloned().collect();
        for k in keys {
            if !plain.contains_key(&k) {
                changed = self.get_container_mut(k).apply_changes(ctx, None)? || changed;
            }
        }
        for (k, v) in plain {
            changed = self.get_container_mut(k).apply_changes(ctx, Some(v))? || changed;
        }
        Ok(changed)
    }
}

impl<'d, K, V, T, N> DeltaMut<'d, LwwMap<K, V, T, N>>
where
    K: Ord,
    N: Ord + Clone,
{
    /// Converts this into a mutable reference to the container associated with `key` from this map.
    ///
    /// Same as [`LwwMapWrite::get_container_mut`], but consumes self to allow the lifetime to be
    /// propagated to the returned value.
    pub fn into_container_mut(self, key: K) -> DeltaMut<'d, LwwCrdtContainer<V, T, N>> {
        let base = self.base.and_then(|base| base.elements.get(&key));
        let delta = self.delta.elements.entry(key).or_default();
        DeltaMut { base, delta }
    }
}

impl<K, V, T, N> CrdtState for LwwMap<K, V, T, N>
where
    K: Ord + Clone,
    V: CrdtUnion<N> + Clone,
    T: TotalTimestamp + Clone,
    N: Ord + Clone,
{
    fn merge(a: &Self, b: &Self) -> Self {
        Self {
            elements: zip_btree_map(&a.elements, &b.elements)
                .map(|(key, item)| {
                    (
                        key.clone(),
                        match item {
                            ZipItem::Left(value) | ZipItem::Right(value) => value.clone(),
                            ZipItem::Both(value_a, value_b) => CrdtState::merge(value_a, value_b),
                        },
                    )
                })
                .collect(),
        }
    }

    #[cfg(any(test, feature = "checker"))]
    fn is_valid_collection<'a>(collection: impl IntoIterator<Item = &'a Self>) -> bool
    where
        Self: 'a,
    {
        LwwCrdtContainer::is_valid_collection(
            collection.into_iter().flat_map(|s| s.elements.values()),
        )
    }
}

impl<K, V, T, N, C> LwwMapRead<K, V, T, N> for C
where
    K: Ord + 'static,
    V: 'static,
    T: 'static,
    N: Ord + 'static,
    C: AsDeltaRef<LwwMap<K, V, T, N>>,
{
}
impl<K, V, T, N, C> LwwMapWrite<K, V, T, N> for C
where
    K: Ord + 'static,
    V: 'static,
    T: 'static,
    N: Ord + Clone + 'static,
    C: AsDeltaMut<LwwMap<K, V, T, N>>,
{
}

#[cfg(feature = "proto")]
mod proto {
    use distributed_time::hybrid_logical_clock::HybridLogicalTimestamp;
    use submerge_internal_proto::{FromProto, FromProtoError, NodeMapping, ToProto};

    use crate::{lww_crdt_container::LwwCrdtContainer, typed_crdt::TypedCrdt};

    use super::LwwMap;

    impl FromProto
        for LwwMap<String, TypedCrdt<Vec<u8>, String>, HybridLogicalTimestamp<String>, String>
    {
        type Proto = submerge_internal_proto::protos::submerge::SubmergeMap;

        fn from_proto(proto: &Self::Proto, node_ids: &[String]) -> Result<Self, FromProtoError> {
            Ok(Self {
                elements: proto
                    .elements
                    .iter()
                    .map(|(key, container)| {
                        Ok((
                            key.clone(),
                            LwwCrdtContainer::from_proto(container, node_ids)?,
                        ))
                    })
                    .collect::<Result<_, _>>()?,
            })
        }
    }

    impl ToProto
        for LwwMap<String, TypedCrdt<Vec<u8>, String>, HybridLogicalTimestamp<String>, String>
    {
        type Proto = submerge_internal_proto::protos::submerge::SubmergeMap;

        fn to_proto(&self, node_ids: &mut NodeMapping<String>) -> Self::Proto {
            submerge_internal_proto::protos::submerge::SubmergeMap {
                elements: self
                    .elements
                    .iter()
                    .map(|(node, container)| (node.clone(), container.to_proto(node_ids)))
                    .collect(),
                ..Default::default()
            }
        }
    }

    #[cfg(test)]
    #[derive_fuzztest::proptest]
    fn map_roundtrip(
        map: LwwMap<String, TypedCrdt<Vec<u8>, String>, HybridLogicalTimestamp<String>, String>,
    ) {
        let mut node_ids = NodeMapping::default();
        assert_eq!(
            LwwMap::from_proto(&map.to_proto(&mut node_ids), &node_ids.into_vec()).unwrap(),
            map
        );
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{
        checker::test_fakes::{FakeContext, TriState},
        lww_map::{LwwMapRead, LwwMapWrite},
        register::{RegisterRead, RegisterWrite},
    };
    use distributed_time::{
        hybrid_logical_clock::HybridLogicalTimestamp, vector_clock::VectorClock,
    };

    use crate::{
        lww_crdt_container::LwwCrdtContainerApplyError,
        register::Register,
        typed_crdt::{PlainTypes, TypedCrdt, TypedError},
        vector_data::VectorDataError,
    };

    use super::LwwMap;

    #[test]
    fn test_set_value() {
        let mut map: LwwMap<TriState, TypedCrdt<&'static str, u8>, HybridLogicalTimestamp<u8>, u8> =
            LwwMap::default();
        let mut reg = Register::<&'static str, VectorClock<u8>>::default();
        reg.set(&FakeContext::new(1, 1_u8), "1234").unwrap();
        map.set_value(&FakeContext::new(1, 1_u8), TriState::A, reg.into())
            .unwrap();

        assert_eq!(
            &"1234",
            map.get_child::<Register::<&'static str, VectorClock<u8>>>(&TriState::A)
                .unwrap()
                .get()
                .unwrap()
        );
    }

    #[test]
    fn test_apply_changes() {
        let mut map: LwwMap<TriState, TypedCrdt<TriState, u8>, HybridLogicalTimestamp<u8>, u8> =
            LwwMap::default();
        let plain = BTreeMap::from_iter([(
            TriState::A,
            PlainTypes::VectorData(BTreeMap::from_iter([(1, TriState::A)])),
        )]);
        let _ = map
            .apply_changes(&FakeContext::new(1, 1_u8), plain.clone())
            .unwrap();

        assert_eq!(map.to_plain(), plain);
    }

    #[test]
    fn test_apply_changes_vectordata_mismatch() {
        let mut map: LwwMap<TriState, TypedCrdt<TriState, u8>, HybridLogicalTimestamp<u8>, u8> =
            LwwMap::default();
        let plain = BTreeMap::from_iter([(
            TriState::A,
            PlainTypes::VectorData(BTreeMap::from_iter([(2, TriState::A)])),
        )]);
        assert_eq!(
            LwwCrdtContainerApplyError::ChildError(TypedError::VectorData(
                VectorDataError::MismatchedNode
            )),
            map.apply_changes(&FakeContext::new(1, 1_u8), plain.clone())
                .unwrap_err()
        );
    }
}
