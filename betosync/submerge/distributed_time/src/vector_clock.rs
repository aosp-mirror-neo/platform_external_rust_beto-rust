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

//! A vector clock implementation.
//!
//! See the documentation for [`VectorClock`].

use std::{cmp::Ordering, collections::BTreeMap, hash::Hash};

use arbitrary::Arbitrary;
use derive_where::derive_where;
use serde::{Deserialize, Serialize};

use crate::{DistributedClock, NonSemanticOrd, TimestampOverflow};

/// A vector clock which offers partial ordering of events.
///
/// See also: <https://en.wikipedia.org/wiki/Vector_clock>
///
/// # Params
/// - `N`: The node ID.
#[derive(Debug, Clone, Arbitrary, Serialize, Deserialize)]
#[derive_where(Default)]
#[serde(transparent)]
pub struct VectorClock<N: Ord> {
    dots: BTreeMap<N, u32>,
}

impl<N: Ord> VectorClock<N> {
    /// Create a new instance from the given iterator or (node_id, clock_value) pairs.
    pub fn new(dots: impl IntoIterator<Item = (N, u32)>) -> Self {
        Self {
            dots: dots.into_iter().collect(),
        }
    }

    /// Convert this clock into its underlying map of "dots", which are the (node, version number)
    /// pairs that represent what events happened before this clock.
    pub fn into_dots(self) -> BTreeMap<N, u32> {
        self.dots
    }
}

impl<N: Ord> NonSemanticOrd for VectorClock<N> {
    fn non_semantic_cmp(&self, other: &Self) -> Ordering {
        self.dots.cmp(&other.dots)
    }
}

impl<N: Ord + Hash> Hash for VectorClock<N> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.dots.hash(state);
    }
}

impl<N: Ord> PartialEq for VectorClock<N> {
    fn eq(&self, other: &Self) -> bool {
        self.dots
            .iter()
            .all(|(k, v)| v == other.dots.get(k).unwrap_or(&0))
            && other
                .dots
                .iter()
                .all(|(k, v)| self.dots.get(k).unwrap_or(&0) == v)
    }
}

impl<N: Ord> Eq for VectorClock<N> {}

impl<N: Ord> PartialOrd for VectorClock<N> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let mut compare_results = self.dots.keys().chain(other.dots.keys()).map(|k| {
            self.dots
                .get(k)
                .unwrap_or(&0)
                .cmp(other.dots.get(k).unwrap_or(&0))
        });
        compare_results.try_fold(Ordering::Equal, |acc, r| match (acc, r) {
            (Ordering::Greater, Ordering::Less) => None,
            (Ordering::Less, Ordering::Greater) => None,
            (Ordering::Equal, r) => Some(r),
            _ => Some(acc),
        })
    }
}

impl<N> DistributedClock for VectorClock<N>
where
    N: Ord + Clone,
{
    type NodeId = N;

    fn increment(&self, updater: &N) -> Result<Self, TimestampOverflow> {
        let mut copy = self.dots.clone();
        let _ = copy.insert(
            updater.clone(),
            copy.get(updater)
                .unwrap_or(&0)
                .checked_add(1)
                .ok_or(TimestampOverflow)?,
        );
        Ok(Self { dots: copy })
    }

    fn least_upper_bound_in_place(&mut self, other: &Self) {
        for (k, v) in &other.dots {
            let self_value = self.dots.entry(k.clone()).or_default();
            *self_value = *v.max(self_value);
        }
    }
}

#[cfg(feature = "proto")]
mod proto {
    use submerge_internal_proto::{FromProto, FromProtoError, NodeMapping, ToProto};

    use super::VectorClock;

    impl FromProto for VectorClock<String> {
        type Proto = submerge_internal_proto::protos::submerge::CompressedVectorClock;

        fn from_proto(proto: &Self::Proto, node_ids: &[String]) -> Result<Self, FromProtoError> {
            if proto.nodes.len() != proto.values.len() {
                return Err(FromProtoError::MismatchedVectorClockLength);
            }
            Ok(Self {
                dots: proto
                    .nodes
                    .iter()
                    .zip(proto.values.iter())
                    .map(|(n, v)| {
                        Ok((
                            node_ids
                                .get(*n as usize)
                                .ok_or(FromProtoError::MissingNodeId)?
                                .clone(),
                            *v,
                        ))
                    })
                    .collect::<Result<_, _>>()?,
            })
        }
    }

    impl VectorClock<String> {
        /// Construct a new `VectorClock` from its transferable proto representation.
        pub fn from_transferable_proto(
            proto: &submerge_internal_proto::protos::submerge::VectorClock,
        ) -> Result<Self, FromProtoError> {
            Ok(Self {
                dots: proto.node_clock_values.clone().into_iter().collect(),
            })
        }

        /// Convert this `VectorClock` to its transferable proto representation.
        pub fn to_transferable_proto(
            &self,
        ) -> submerge_internal_proto::protos::submerge::VectorClock {
            submerge_internal_proto::protos::submerge::VectorClock {
                node_clock_values: self.dots.clone().into_iter().collect(),
                ..Default::default()
            }
        }
    }

    impl ToProto for VectorClock<String> {
        type Proto = submerge_internal_proto::protos::submerge::CompressedVectorClock;

        fn to_proto(&self, node_ids: &mut NodeMapping<String>) -> Self::Proto {
            let (nodes, values) = self
                .dots
                .iter()
                .map(|(node, v)| (node_ids.get_index(node), *v))
                .unzip();
            submerge_internal_proto::protos::submerge::CompressedVectorClock {
                nodes,
                values,
                ..Default::default()
            }
        }
    }

    #[cfg(test)]
    #[derive_fuzztest::proptest]
    #[allow(clippy::unwrap_used)]
    fn vector_clock_round_trip(vector_clock: VectorClock<String>) {
        let mut node_ids = NodeMapping::default();
        assert_eq!(
            VectorClock::from_proto(&vector_clock.to_proto(&mut node_ids), &node_ids.into_vec())
                .unwrap(),
            vector_clock
        );
        assert_eq!(
            VectorClock::from_transferable_proto(&vector_clock.to_transferable_proto()).unwrap(),
            vector_clock
        );
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::DistributedClock;

    use super::VectorClock;

    #[test]
    fn test_inc() {
        let mut dots = BTreeMap::new();
        let _ = dots.insert(0, 0);
        let _ = dots.insert(1, 0);
        let _ = dots.insert(2, 0);
        let clock = VectorClock { dots };
        let incremented = clock.increment(&0).unwrap();

        let mut expected_dots: BTreeMap<u16, u32> = BTreeMap::new();
        let _ = expected_dots.insert(0, 1);
        let _ = expected_dots.insert(1, 0);
        let _ = expected_dots.insert(2, 0);

        assert_eq!(expected_dots, incremented.dots);
    }
}
