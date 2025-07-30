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

//! The hybrid logical clock is a hybrid between a wall clock (which captures physical time), and a
//! logical clock (which captures causality).
//!
//! See the documentation for [`HybridLogicalTimestamp`].

use std::fmt::Debug;

use arbitrary::Arbitrary;
use derive_where::derive_where;
use serde::{Deserialize, Serialize};

use crate::{TimestampOverflow, TotalTimestamp, WallTimestampProvider};

/// The hybrid logical clock is a hybrid between a wall clock (which captures physical time), and a
/// logical clock (which captures causality).
///
/// The usage is similar to a [Lamport timestamp](https://en.wikipedia.org/wiki/Lamport_timestamp),
/// which can be compared between any pair of timestamps and get a total order, such that if a
/// happens-before b, then HLC(a) < HLC(b).
///
/// If two events are concurrent (neither happens before the other), we do not know whether HLC(a)
/// or HLC(b) is larger, but the event with the later physical time (as determined by the device's
/// local clock) will be more likely to be greater.
///
/// This timestamp is also tagged with its updater, guaranteeing uniqueness (i.e. HLC(a) == HLC(b)
/// implies a == b).
///
/// See this paper for details and proofs about its properties:
/// <https://cse.buffalo.edu/tech-reports/2014-04.pdf>.
///
/// # Params
/// - `W`: The wall-clock timestamp provider that is used to get the current wall-clock time. HLC
///        assumes that this wall clock time can be skewed and changed in any direction, and it
///        won't cause HLC to violate causality. However, for concurrent events, the smaller the
///        clock skew, the more likely it is for this timestamp to be consistent with the user's
///        observed sequence of events.
#[derive(Debug, Serialize, Deserialize, Arbitrary)]
#[derive_where(Clone, PartialOrd, Ord, PartialEq, Eq, Hash; N, UnnamedHybridLogicalTimestamp)]
#[serde(bound(serialize = "N: Serialize", deserialize = "N: Deserialize<'de>"))]
pub struct HybridLogicalTimestamp<N>
where
    N: Ord + Clone,
{
    time: UnnamedHybridLogicalTimestamp,
    updater: N,
}

impl<N> TotalTimestamp for HybridLogicalTimestamp<N>
where
    N: Ord + Clone,
{
    type NodeId = N;

    fn new_with_context(updater: &N, timestamp_provider: &impl WallTimestampProvider) -> Self {
        Self {
            time: UnnamedHybridLogicalTimestamp::new(timestamp_provider, 0),
            updater: updater.clone(),
        }
    }

    fn increment(
        &self,
        updater: &N,
        timestamp_provider: &impl WallTimestampProvider,
    ) -> Result<Self, TimestampOverflow> {
        Ok(Self {
            time: self.time.increment(timestamp_provider)?,
            updater: updater.clone(),
        })
    }
}

/// The unnamed version of HLC which does not record the updater node. This is not unique by itself
/// (i.e. HLC(a) == HLC(b) does not imply a == b).
///
/// If you need the uniqueness property, use [`HybridLogicalTimestamp`] instead.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Arbitrary)]
pub(crate) struct UnnamedHybridLogicalTimestamp {
    /// The logical time of this timestamp. When incremented, the incrementing node will take the
    /// `max(current.logical_time, local_physical_time)`, thus preserving the monotonicity of the
    /// values.
    pub logical_time: u64,
    /// The causality counter used to track the causality between different clocks that has the same
    /// logical time. This can be non-zero due to clock skew, where the node received an event with
    /// a "future" timestamp and tries to increment it. In that case the new `logical_time` will be
    /// the same as the old `logical_time`, but this `causality` field will be incremented to make
    /// sure the timestamp is greater than the old one.
    pub causality: u8,
}

impl UnnamedHybridLogicalTimestamp {
    /// Increment the timestamp.
    ///
    /// This ensures that the returned timestamp H' > self, and H' >= time_context.now().
    pub fn increment(
        &self,
        time_provider: &impl WallTimestampProvider,
    ) -> Result<Self, TimestampOverflow> {
        let logical_time = self.logical_time.max(time_provider.now());
        let causality = if logical_time == self.logical_time {
            self.causality.checked_add(1).ok_or(TimestampOverflow)?
        } else {
            0
        };
        Ok(Self {
            logical_time,
            causality,
        })
    }

    /// Create a new timestamp at the given time context.
    pub fn new(time_provider: &impl WallTimestampProvider, causality: u8) -> Self {
        Self {
            logical_time: time_provider.now(),
            causality,
        }
    }
}

#[cfg(feature = "proto")]
mod proto {
    use submerge_internal_proto::{FromProto, FromProtoError, NodeMapping, ToProto};

    use super::{HybridLogicalTimestamp, UnnamedHybridLogicalTimestamp};

    impl FromProto for UnnamedHybridLogicalTimestamp {
        type Proto = submerge_internal_proto::protos::submerge::HybridLogicalTimestamp;

        fn from_proto(proto: &Self::Proto, _node_ids: &[String]) -> Result<Self, FromProtoError> {
            Ok(Self {
                logical_time: proto.logical_time,
                causality: proto
                    .causality
                    .try_into()
                    .map_err(|_| FromProtoError::NumericOverflow)?,
            })
        }
    }

    impl ToProto for UnnamedHybridLogicalTimestamp {
        type Proto = submerge_internal_proto::protos::submerge::HybridLogicalTimestamp;

        fn to_proto(&self, _node_ids: &mut NodeMapping<String>) -> Self::Proto {
            submerge_internal_proto::protos::submerge::HybridLogicalTimestamp {
                logical_time: self.logical_time,
                causality: self.causality.into(),
                ..Default::default()
            }
        }
    }

    impl HybridLogicalTimestamp<String> {
        /// Construct a proto from this timestamp.
        ///
        /// Returns a pair `(HybridLogicalTimestampProto, updater)`, where `updater` is the index of the
        /// updater in `NodeMapping`.
        pub fn to_proto(
            &self,
            node_ids: &mut NodeMapping<String>,
        ) -> (
            submerge_internal_proto::protos::submerge::HybridLogicalTimestamp,
            u32,
        ) {
            (
                self.time.to_proto(node_ids),
                node_ids.get_index(&self.updater),
            )
        }

        /// Construct a timestamp from the given proto and the updater.
        pub fn from_proto(
            proto: &submerge_internal_proto::protos::submerge::HybridLogicalTimestamp,
            updater: u32,
            node_ids: &[String],
        ) -> Result<Self, FromProtoError> {
            Ok(HybridLogicalTimestamp {
                time: UnnamedHybridLogicalTimestamp::from_proto(proto, node_ids)?,
                updater: node_ids
                    .get(updater as usize)
                    .ok_or(FromProtoError::MissingNodeId)?
                    .clone(),
            })
        }
    }

    #[cfg(test)]
    #[derive_fuzztest::proptest]
    fn unnamed_hlc_round_trip(hlc: UnnamedHybridLogicalTimestamp) {
        let mut node_ids = NodeMapping::default();
        assert_eq!(
            UnnamedHybridLogicalTimestamp::from_proto(
                &hlc.to_proto(&mut node_ids),
                &node_ids.into_vec()
            )
            .unwrap(),
            hlc
        );
    }

    #[cfg(test)]
    #[derive_fuzztest::proptest]
    fn hlc_round_trip(hlc: HybridLogicalTimestamp<String>) {
        let mut node_ids = NodeMapping::default();
        let (timestamp_proto, updater) = hlc.to_proto(&mut node_ids);
        assert_eq!(
            HybridLogicalTimestamp::from_proto(&timestamp_proto, updater, &node_ids.into_vec())
                .unwrap(),
            hlc
        );
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use crate::fake_timestamp::FakeTimestamp;

    use super::UnnamedHybridLogicalTimestamp;

    #[test]
    fn test_unnamed_hybrid_logical_timestamp_ord() {
        let t1 = UnnamedHybridLogicalTimestamp::new(&FakeTimestamp(1_u8), 0);
        let t2 = UnnamedHybridLogicalTimestamp::new(&FakeTimestamp(2_u8), 0);
        assert_eq!(Ordering::Less, t1.cmp(&t2));
        assert!(t1 < t2);
    }
}
