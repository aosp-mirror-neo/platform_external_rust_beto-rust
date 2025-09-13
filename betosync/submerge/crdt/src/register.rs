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

//! A register can store any serializable type and updates atomically.
//!
//! Each register value is associated with timestamps such that clients can either get the last
//! written value, or get a list of all concurrent values ordered by wall-clock timestamps.

use std::{cmp::Ordering, collections::BTreeSet, fmt::Debug};

use crate::{
    delta::{AsDeltaMut, AsDeltaRef},
    utils::{merge_descending_iters, Either},
    ContentEq, CrdtState, UpdateContext,
};
use arbitrary::{Arbitrary, Unstructured};
use derive_where::derive_where;
use distributed_time::{
    compound_timestamp::CompoundTimestamp, DistributedClock, NonSemanticOrd, TimestampOverflow,
    TotalTimestamp,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Arbitrary, Serialize, Deserialize)]
#[derive_where(PartialEq, Eq, PartialOrd, Ord; T)]
struct RegEntry<V, T> {
    #[derive_where(skip)]
    value: V,
    timestamp: T,
}

/// A register can store any serializable type and updates atomically without merging.
///
/// Each register value is associated with timestamps such that clients can either get the last
/// written value (using [`get`][RegisterRead::get]), or get a list of all concurrent values ordered
/// by a [`CompoundTimestamp`] (using [`get_all`][RegisterRead::get_all]).
///
/// # Params
/// - `V`: The value type
/// - `D`: The logical component of the timestamp, which provides the causality order for values in
///        this register.
/// - `W`: Provides the current wall clock time, used to tie-break concurrent events.
///
/// # Implementations
/// * See [`RegisterRead`] for methods on read-only references of [`Register`].
/// * See [`RegisterWrite`] for methods on mutable references of [`Register`].
#[derive(Debug, Serialize, Deserialize)]
#[derive_where(Default)]
#[derive_where(PartialEq, Eq, Clone; V, CompoundTimestamp<D>)]
#[serde(bound(
    serialize = "V: Serialize, D: Serialize",
    deserialize = "V: Deserialize<'de>, D: Deserialize<'de> + NonSemanticOrd"
))]
#[serde(transparent)]
pub struct Register<V, D>
where
    D: DistributedClock,
{
    elements: BTreeSet<RegEntry<V, CompoundTimestamp<D>>>,
}

impl<'a, V, D> Arbitrary<'a> for Register<V, D>
where
    V: Arbitrary<'a>,
    D: DistributedClock + NonSemanticOrd + Default + Arbitrary<'a>,
{
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let values: BTreeSet<RegEntry<V, CompoundTimestamp<D>>> = u.arbitrary()?;
        let mut clock = D::default();
        Ok(Self {
            elements: values
                .into_iter()
                .rev()
                // Iterate through the elements in descending order of the timestamp. If the entry
                // is less than `clock` (the least-upper-bound of all the entries we've seen so
                // far), that entry is superseded and should not have made it into the register
                // (`merge` would have filtered those out)
                .filter(move |entry| {
                    let order = entry.timestamp.distributed_clock.partial_cmp(&clock);
                    clock.least_upper_bound_in_place(&entry.timestamp.distributed_clock);
                    order != Some(Ordering::Less)
                })
                .collect(),
        })
    }
}

impl<V, D> Register<V, D>
where
    V: Clone,
    D: DistributedClock + NonSemanticOrd + Default + Clone,
{
    /// Create a new register with the given value.
    pub fn new(ctx: &impl UpdateContext<D::NodeId>, value: V) -> Result<Self, TimestampOverflow> {
        let timestamp =
            CompoundTimestamp::new_with_context(ctx.updater(), ctx.timestamp_provider());
        Ok(Self {
            elements: BTreeSet::from_iter([RegEntry { timestamp, value }]),
        })
    }

    fn merge_elements(
        a: &BTreeSet<RegEntry<V, CompoundTimestamp<D>>>,
        b: &BTreeSet<RegEntry<V, CompoundTimestamp<D>>>,
    ) -> BTreeSet<RegEntry<V, CompoundTimestamp<D>>> {
        let mut clock = D::default();
        let merged_iter =
            merge_descending_iters(a.iter().rev(), b.iter().rev()).filter(move |item| {
                let order = item.timestamp.distributed_clock.partial_cmp(&clock);
                clock.least_upper_bound_in_place(&item.timestamp.distributed_clock);
                order != Some(Ordering::Less)
            });
        merged_iter.cloned().collect()
    }
}

fn iter_descending<V, D, R>(
    register: &R,
) -> impl Iterator<Item = &RegEntry<V, CompoundTimestamp<D>>>
where
    V: 'static,
    D: DistributedClock + NonSemanticOrd + Default + 'static,
    R: RegisterRead<V, D> + ?Sized,
{
    let iter_opt = match (register.base(), register.delta()) {
        (None, None) => None,
        (None, Some(reg)) | (Some(reg), None) => Some(Either::Left(reg.elements.iter().rev())),
        (Some(base), Some(delta)) => {
            let mut version_vector = D::default();
            Some(Either::Right(
                merge_descending_iters(base.elements.iter().rev(), delta.elements.iter().rev())
                    .filter(move |value| {
                        // filter out values that have been superseded
                        let comparison = value
                            .timestamp
                            .distributed_clock
                            .partial_cmp(&version_vector);
                        version_vector
                            .least_upper_bound_in_place(&value.timestamp.distributed_clock);
                        comparison != Some(std::cmp::Ordering::Less)
                    }),
            ))
        }
    };
    iter_opt.into_iter().flat_map(|e| e.into_iter())
}

/// Read-only operations for this Register CRDT.
///
/// ## See also
/// * See [`Register`] for a description of this register type.
/// * See [`RegisterWrite`] for mutating operations on this register type.
pub trait RegisterRead<V, D>: AsDeltaRef<Register<V, D>>
where
    D: DistributedClock,
{
    /// Returns a vec containing the values associated with all concurrent changes, ordered from
    /// newest to oldest. The returned vec may be empty if the register have just been initialized
    /// with `default` and have never been written to.
    fn get_all(&self) -> Vec<&V>
    where
        V: 'static,
        D: DistributedClock + NonSemanticOrd + Default + 'static,
    {
        iter_descending(self)
            .map(|RegEntry { value, .. }| value)
            .collect()
    }

    /// Get the latest value from the register according to the timestamps.
    fn get(&self) -> Option<&V>
    where
        V: 'static,
        D: DistributedClock + NonSemanticOrd + Default + 'static,
    {
        iter_descending(self)
            .map(|RegEntry { value, .. }| value)
            .next()
    }

    /// Copies the data without the associated metadata and returns the plain type.
    ///
    /// See also: [`crate::HasPlainRepresentation`].
    fn to_plain(&self) -> Vec<V>
    where
        V: Clone + 'static,
        D: DistributedClock + NonSemanticOrd + Default + 'static,
    {
        self.get_all().into_iter().cloned().collect()
    }
}

/// Mutating operations for this Register CRDT.
///
/// ## See also
/// * See [`Register`] for a description of this register type.
/// * See [`RegisterRead`] for read-only operations of this register type.
pub trait RegisterWrite<V, D>: RegisterRead<V, D> + AsDeltaMut<Register<V, D>>
where
    D: DistributedClock + Eq,
{
    /// Set the value of this register.
    ///
    /// This sets the register value as a single unit. When merged, it either takes this value `V`,
    /// or another value `V` from another `set` call, never combining the results from different
    /// `V`s.
    ///
    /// This overrides all values currently in the register, implicitly resolving all "conflicts"
    /// from concurrent modifications that happened before.
    fn set(
        &mut self,
        ctx: &impl UpdateContext<D::NodeId>,
        value: V,
    ) -> Result<(), TimestampOverflow>
    where
        V: 'static,
        D: NonSemanticOrd + Default + Clone + 'static,
    {
        // Merge all of the timestamps to form the basis of our new timestamp, since we have
        // observed those values.
        let timestamp = match CompoundTimestamp::least_upper_bound(
            iter_descending(self).map(|entry| &entry.timestamp),
        ) {
            Some(t) => t.increment(ctx.updater(), ctx.timestamp_provider())?,
            None => CompoundTimestamp::new_with_context(ctx.updater(), ctx.timestamp_provider()),
        };
        *self.delta_mut() = Register {
            elements: BTreeSet::from_iter([RegEntry { timestamp, value }]),
        };
        Ok(())
    }

    /// Apply the changes in the plain representation into this CRDT. This assumes that all changes
    /// in the plain representation were made by the calling node, updating the associated CRDT
    /// metadata in the process.
    ///
    /// After applying the changes, [`to_plain`][RegisterRead::to_plain] should return the same
    /// value as `plain`.
    ///
    /// # Returns
    ///
    /// `true` if applying the change results in an update in the CRDT state, or false if Returns
    /// true if applying the change results in an update in the CRDT state, or false if `plain` is
    /// the same as [`to_plain`][RegisterRead::to_plain] to begin with. This can be used to avoid
    /// sending unnecessary update messages over the network, or writing changes to disk
    /// unnecessarily.
    fn apply_changes(
        &mut self,
        ctx: &impl UpdateContext<D::NodeId>,
        mut plain: Vec<V>,
    ) -> Result<bool, RegisterError>
    where
        V: PartialEq + 'static,
        D: Default + NonSemanticOrd + Clone + 'static,
    {
        if iter_descending(self)
            .map(|RegEntry { value, .. }| value)
            .ne(plain.as_slice())
        {
            if plain.len() != 1 {
                // When changing the value, only one value is allowed. Since timestamp data is
                // tagged per-node, allowing multiple values to be written may violate the global
                // uniqueness of the timestamps.
                return Err(RegisterError::UnexpectedLength);
            }
            self.set(ctx, plain.remove(0))?;
            Ok(true)
        } else {
            // If the values did not change, no need to check for the length, just keep all existing
            // concurrent values.
            Ok(false)
        }
    }
}

impl<V, D> CrdtState for Register<V, D>
where
    V: Clone,
    D: DistributedClock + NonSemanticOrd + Clone + Default,
{
    fn merge(a: &Self, b: &Self) -> Result<Self, crate::MergeError> {
        Ok(Self {
            elements: Self::merge_elements(&a.elements, &b.elements),
        })
    }

    #[cfg(any(test, feature = "checker"))]
    fn is_valid_collection<'a>(collection: impl IntoIterator<Item = &'a Self>) -> bool
    where
        Self: 'a,
    {
        use crate::checker::utils::IterExt;

        let mut timestamps = collection
            .into_iter()
            .flat_map(|s| s.elements.iter().map(|v| &v.timestamp));
        let is_comparable = timestamps
            .iter_pairs_unordered()
            .all(|(v1, v2)| CompoundTimestamp::is_comparable(v1, v2));
        is_comparable
    }
}

impl<V, D> ContentEq for Register<V, D>
where
    V: PartialEq,
    D: DistributedClock + PartialEq,
    CompoundTimestamp<D>: PartialEq,
{
    fn content_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl<T: AsDeltaRef<Register<V, D>>, V, D: DistributedClock> RegisterRead<V, D> for T {}
impl<T: AsDeltaMut<Register<V, D>>, V, D: DistributedClock> RegisterWrite<V, D> for T {}

/// Error in [`RegisterWrite::apply_changes`].
#[derive(Debug, PartialEq, Eq)]
pub enum RegisterError {
    /// The list passed into `apply_changes` must have length 1.
    UnexpectedLength,
    /// The operation cannot be performed because the associated timestamp will overflow.
    TimestampOverflow,
}

impl From<TimestampOverflow> for RegisterError {
    fn from(_: TimestampOverflow) -> Self {
        RegisterError::TimestampOverflow
    }
}

/// Checker to help implement invariant tests over arbitrary operations on a register.
///
/// Requires the feature _`checker`_.
#[cfg(any(test, feature = "checker"))]
pub mod checker {
    use std::fmt::Debug;

    use arbitrary::Arbitrary;
    use distributed_time::vector_clock::VectorClock;

    use crate::{
        checker::simulation::{Operation, SimulationContext},
        delta::DeltaMut,
        register::{Register, RegisterRead, RegisterWrite},
    };

    /// Mutation operations on a [`RegisterWrite`].
    ///
    /// Can be used with [`Arbitrary`] to generate arbitrary operations to be applied on the
    /// register.
    #[derive(Debug, Clone, Arbitrary)]
    pub enum RegOp<N, V> {
        /// Represents the [`RegisterWrite::set`] operation.
        Set {
            /// The ID of the node setting this value.
            node_id: N,
            /// The value to be set.
            value: V,
        },
        /// Represents the [`RegisterWrite::apply_changes`] operation.
        ApplyChanges {
            /// The ID of the node applying the changes.
            node_id: N,
            /// The list of values to be set.
            plain: Vec<V>,
        },
    }

    impl<N, V> Operation<Register<V, VectorClock<N>>, N> for RegOp<N, V>
    where
        N: Ord + Clone + 'static,
        V: Clone + Eq + Debug + 'static,
    {
        fn apply(
            self,
            mut state: DeltaMut<Register<V, VectorClock<N>>>,
            ctx: &SimulationContext<N>,
        ) {
            match self {
                RegOp::Set { node_id, value } => {
                    let result = state.set(&ctx.context(&node_id), value.clone());
                    if result.is_ok() {
                        assert_eq!(Some(&value), state.get());
                        assert_eq!(vec![&value], state.get_all());
                    }
                }
                RegOp::ApplyChanges { node_id, plain } => {
                    let ctx = ctx.context(&node_id);
                    if state.apply_changes(&ctx, plain.clone()).is_ok() {
                        assert_eq!(plain, state.to_plain());
                    }
                }
            }
        }
    }
}

#[cfg(feature = "proto")]
mod proto {
    use distributed_time::{compound_timestamp::CompoundTimestamp, vector_clock::VectorClock};
    use submerge_internal_proto::{FromProto, FromProtoError, NodeMapping, ToProto};

    use super::{RegEntry, Register};

    impl FromProto for Register<Vec<u8>, VectorClock<String>> {
        type Proto = submerge_internal_proto::protos::submerge::SubmergeRegister;

        fn from_proto(proto: &Self::Proto, node_ids: &[String]) -> Result<Self, FromProtoError> {
            Ok(Self {
                elements: proto
                    .elements
                    .iter()
                    .map(|elem| {
                        Ok(RegEntry {
                            value: elem
                                .value
                                .clone()
                                .ok_or(FromProtoError::MissingRequiredField)?,
                            timestamp: CompoundTimestamp::from_proto(
                                &elem.hlc,
                                &elem.vector_clock,
                                node_ids,
                            )?,
                        })
                    })
                    .collect::<Result<_, _>>()?,
            })
        }
    }

    impl ToProto for Register<Vec<u8>, VectorClock<String>> {
        type Proto = submerge_internal_proto::protos::submerge::SubmergeRegister;

        fn to_proto(&self, node_ids: &mut NodeMapping<String>) -> Self::Proto {
            submerge_internal_proto::protos::submerge::SubmergeRegister {
                elements: self
                    .elements
                    .iter()
                    .map(|reg_entry| {
                        let (hlc, vector_clock) = reg_entry.timestamp.to_proto(node_ids);
                        submerge_internal_proto::protos::submerge::submerge_register::RegisterElement {
                            value: Some(reg_entry.value.clone()),
                            vector_clock: Some(vector_clock).into(),
                            hlc: Some(hlc).into(),
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
    fn register_roundtrip(register: Register<Vec<u8>, VectorClock<String>>) {
        let mut node_ids = NodeMapping::default();
        assert_eq!(
            Register::from_proto(&register.to_proto(&mut node_ids), &node_ids.into_vec()).unwrap(),
            register
        );
    }
}

#[cfg(test)]
mod tests {
    use super::Register;
    use crate::{
        checker::test_fakes::FakeContext,
        register::{RegisterRead, RegisterWrite},
        CrdtState,
    };
    use distributed_time::{vector_clock::VectorClock, TimestampOverflow};
    use serde_json::json;

    type TestRegister = Register<u8, VectorClock<u8>>;

    #[test]
    fn test_set() {
        let r: TestRegister = Register::new(&FakeContext::new(0, 0_u8), 1).unwrap();

        assert_eq!(vec![&1], r.get_all());
        assert_eq!(Some(&1), r.get());
    }

    #[test]
    fn test_set_concurrent() -> Result<(), TimestampOverflow> {
        let r1: TestRegister = Register::new(&FakeContext::new(0, 2_u8), 1)?;

        let r2: TestRegister = Register::new(&FakeContext::new(1, 0_u8), 2)?;

        let r3: TestRegister = Register::new(&FakeContext::new(2, 2_u8), 3)?;

        let r_merged = CrdtState::merge(&r1, &r2).unwrap();
        let r_merged = CrdtState::merge(&r_merged, &r3).unwrap();

        // Ordered from oldest to newest according to timestamp, tie-broken by node ID
        assert_eq!(vec![&3, &1, &2], r_merged.get_all());
        assert_eq!(Some(&3), r_merged.get());

        Ok(())
    }

    #[test]
    fn test_overwrite_value() -> Result<(), TimestampOverflow> {
        let mut r1: TestRegister = Register::new(&FakeContext::new(0, 2_u8), 1)?;
        r1.set(&FakeContext::new(1, 0_u8), 2)?;

        let r2: TestRegister = Register::new(&FakeContext::new(0, 2_u8), 1)?;

        // Merging r2 doesn't change r1 because r1 has already observed that change.
        let r_merged = CrdtState::merge(&r1, &r2).unwrap();

        assert_eq!(vec![&2], r_merged.get_all());
        assert_eq!(Some(&2), r_merged.get());

        Ok(())
    }

    #[test]
    fn test_merge_same() {
        let r1: TestRegister = Register::new(&FakeContext::new(0, 0_u8), 1).unwrap();

        let r_merged = CrdtState::merge(&r1, &r1).unwrap();

        assert_eq!(vec![&1], r_merged.get_all());
        assert_eq!(Some(&1), r_merged.get());
    }

    #[test]
    fn json_decode() {
        let json = json! ([
            {
                "value": "What is your #0 favorite emoji?",
                "timestamp": {
                    "distributed_clock": {},
                    "hybrid_logical_timestamp": {
                        "logical_time": 1,
                        "causality": 0,
                    },
                }
            }
        ]);
        let register = serde_json::from_value::<Register<String, VectorClock<u8>>>(json).unwrap();
        assert_eq!(register.get_all(), vec!["What is your #0 favorite emoji?"])
    }
}
