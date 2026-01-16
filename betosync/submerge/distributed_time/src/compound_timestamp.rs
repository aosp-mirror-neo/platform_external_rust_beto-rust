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

//! A compound timestamp that combines a
//! [`HybridLogicalTimestamp`][crate::hybrid_logical_clock::HybridLogicalTimestamp] with a
//! [`DistributedClock`] (like a vector clock).
//!
//! See the documentation for [`CompoundTimestamp`].

use std::{cmp::Ordering, fmt::Debug};

use arbitrary::Arbitrary;
use derive_where::derive_where;
use serde::{Deserialize, Serialize};

use crate::{
    hybrid_logical_clock::UnnamedHybridLogicalTimestamp, DistributedClock, NonSemanticOrd,
    TimestampOverflow, TotalTimestamp, WallTimestampProvider,
};

/// A compound timestamp that combines a
/// [`HybridLogicalTimestamp`][crate::hybrid_logical_clock::HybridLogicalTimestamp] with a
/// [`DistributedClock`] (like a vector clock).
///
/// The combination provides both last-writer-wins semantics that obeys "potential causality" (which
/// `HybridLogicalTimestamp` provides), and true causality checks (the ability to tell whether two
/// events are concurrent or not) to detect concurrent changes (which are potentially conflicts).
///
/// This is designed to provide the information necessary for both last-writer-wins registers and
/// multi-value registers. This allows us to implement a register that provides both `get()` for
/// last-writer-wins behavior, and `get_all()` for multi-value behavior.
#[derive(Clone, Debug, Serialize, Deserialize, Arbitrary)]
#[derive_where(PartialEq, Eq)]
#[serde(bound(serialize = "D: Serialize", deserialize = "D: Deserialize<'de>"))]
pub struct CompoundTimestamp<D: PartialOrd + Eq> {
    /// The partial-order component of the timestamp. This component is capable of determining
    /// whether two events are concurrent or not, using its `PartialOrd` implementation. If two
    /// events are concurrent, its `partial_cmp` should return `None`.
    pub distributed_clock: D,
    hybrid_logical_timestamp: UnnamedHybridLogicalTimestamp,
}

impl<D> CompoundTimestamp<D>
where
    D: PartialOrd + Eq + NonSemanticOrd,
{
    /// Whether the two given timestamps are comparable.
    ///
    /// For comparable timestamps, the results from comparing the distributed clocks `D` and
    /// comparing the underlying logical timestamp should always be consistent for non-concurrent
    /// events. This implementation guarantees that this always return true except for instances
    /// obtained from the `arbitrary` implementation.
    pub fn is_comparable(a: &Self, b: &Self) -> bool {
        if let Some(distributed_clock_cmp) = a.distributed_clock.partial_cmp(&b.distributed_clock) {
            distributed_clock_cmp == a.hybrid_logical_timestamp.cmp(&b.hybrid_logical_timestamp)
        } else {
            true
        }
    }

    /// Return the causality order of this compound timestamp.
    ///
    /// The causality order is the same as `D::partial_cmp`, which should be the same as
    /// [`Self::cmp`] if the event is not concurrent. Because of the consistency requirements in
    /// [`PartialOrd`] and [`Ord`], `CompoundTimestamp::partial_cmp` never returns `None`. Thus we
    /// have this function to allow extracting the causality information out from this compound
    /// timestamp.
    pub fn causality_order(&self, other: &Self) -> Option<Ordering> {
        // Technically the ordering of the distributed clock should match that of `self`, so the
        // `map` part should not be necessary. But for the simplicity of comparing arbitrarily
        // generated timestamps in invariant testing, we always use `self` as the ordering, and only
        // use `distributed_clock` to detect concurrency.
        self.distributed_clock
            .partial_cmp(&other.distributed_clock)
            .map(|_| self.cmp(other))
    }

    /// Calculate the least-upper-bound of two given timestamps.
    ///
    /// The result `T` in addition to satisfying `T >= a` and `T >= b` (least-upper-bound
    /// definition with respect to `Ord`), this implementation also guarantees that
    /// `causality_order(T, a) matches Some(Greater | Equal)` and `causality_order(T, b) matches
    /// Some(Greater | Equal)` (least-upper-bound definition with respect to the partial order
    /// defined by `causality_order`).
    pub fn least_upper_bound<'a>(iter: impl IntoIterator<Item = &'a Self>) -> Option<Self>
    where
        D: DistributedClock + Clone + 'a,
    {
        let mut acc: Option<(D, &UnnamedHybridLogicalTimestamp)> = None;
        for timestamp in iter {
            match acc {
                Some((ref mut clock, ref mut hlc)) => {
                    clock.least_upper_bound_in_place(&timestamp.distributed_clock);
                    if &timestamp.hybrid_logical_timestamp > hlc {
                        *hlc = &timestamp.hybrid_logical_timestamp;
                    }
                }
                None => {
                    acc = Some((
                        timestamp.distributed_clock.clone(),
                        &timestamp.hybrid_logical_timestamp,
                    ));
                }
            }
        }
        acc.map(|(d, hlc)| Self {
            distributed_clock: d,
            hybrid_logical_timestamp: hlc.clone(),
        })
    }
}

impl<D> PartialOrd for CompoundTimestamp<D>
where
    D: PartialOrd + Eq + NonSemanticOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<D> Ord for CompoundTimestamp<D>
where
    D: PartialOrd + Eq + NonSemanticOrd,
{
    fn cmp(&self, other: &Self) -> Ordering {
        match self
            .hybrid_logical_timestamp
            .cmp(&other.hybrid_logical_timestamp)
        {
            Ordering::Equal => self
                .distributed_clock
                .non_semantic_cmp(&other.distributed_clock),
            order => order,
        }
    }
}

impl<D> TotalTimestamp for CompoundTimestamp<D>
where
    D: DistributedClock + NonSemanticOrd + Default + Clone,
{
    type NodeId = D::NodeId;

    fn increment(
        &self,
        updater: &Self::NodeId,
        timestamp_provider: &impl WallTimestampProvider,
    ) -> Result<Self, TimestampOverflow> {
        Ok(Self {
            distributed_clock: self.distributed_clock.increment(updater)?,
            hybrid_logical_timestamp: self
                .hybrid_logical_timestamp
                .increment(timestamp_provider)?,
        })
    }

    fn new_with_context(
        updater: &Self::NodeId,
        timestamp_provider: &impl WallTimestampProvider,
    ) -> Self {
        Self {
            distributed_clock: D::default().increment(updater).expect(
                "Timestamp should not overflow when incrementing from the default instance",
            ),
            hybrid_logical_timestamp: UnnamedHybridLogicalTimestamp::new(timestamp_provider, 0),
        }
    }
}

#[cfg(feature = "proto")]
mod proto {
    use submerge_internal_proto::{FromProto, FromProtoError, NodeMapping, ToProto};

    use crate::{hybrid_logical_clock::UnnamedHybridLogicalTimestamp, vector_clock::VectorClock};

    use super::CompoundTimestamp;

    impl CompoundTimestamp<VectorClock<String>> {
        /// Construct protos for the given compound timestamp.
        ///
        /// Returns a pair `(HybridLogicalTimestampProto, VectorClockProto)`.
        pub fn to_proto(
            &self,
            node_ids: &mut NodeMapping<String>,
        ) -> (
            submerge_internal_proto::protos::submerge::HybridLogicalTimestamp,
            submerge_internal_proto::protos::submerge::CompressedVectorClock,
        ) {
            (
                self.hybrid_logical_timestamp.to_proto(node_ids),
                self.distributed_clock.to_proto(node_ids),
            )
        }

        /// Construct a compound timestamp from the given protos.
        pub fn from_proto(
            timestamp: &submerge_internal_proto::protos::submerge::HybridLogicalTimestamp,
            vector_clock: &submerge_internal_proto::protos::submerge::CompressedVectorClock,
            node_ids: &[String],
        ) -> Result<Self, FromProtoError> {
            Ok(CompoundTimestamp {
                distributed_clock: VectorClock::from_proto(vector_clock, node_ids)?,
                hybrid_logical_timestamp: UnnamedHybridLogicalTimestamp::from_proto(
                    timestamp, node_ids,
                )?,
            })
        }
    }

    #[cfg(test)]
    #[derive_fuzztest::proptest]
    #[allow(clippy::unwrap_used)]
    fn compound_timestamp_round_trip(compound_timestamp: CompoundTimestamp<VectorClock<String>>) {
        let mut node_ids = NodeMapping::default();
        let (hlc_proto, vector_clock_proto) = compound_timestamp.to_proto(&mut node_ids);
        assert_eq!(
            CompoundTimestamp::from_proto(&hlc_proto, &vector_clock_proto, &node_ids.into_vec())
                .unwrap(),
            compound_timestamp
        );
    }
}

#[cfg(test)]
mod test {
    use std::cmp::Ordering;

    use crate::{
        compound_timestamp::CompoundTimestamp, fake_timestamp::FakeTimestamp,
        hybrid_logical_clock::UnnamedHybridLogicalTimestamp, vector_clock::VectorClock,
        TotalTimestamp,
    };

    #[test]
    fn new_with_context() {
        let t =
            CompoundTimestamp::<VectorClock<u16>>::new_with_context(&0, &FakeTimestamp(123_u32));
        assert_eq!(
            CompoundTimestamp {
                distributed_clock: VectorClock::new([(0, 1)]),
                hybrid_logical_timestamp: UnnamedHybridLogicalTimestamp::new(
                    &FakeTimestamp(123_u32),
                    0
                )
            },
            t
        )
    }

    #[test]
    fn hybrid_timestamp_eq() {
        let t1 = CompoundTimestamp {
            distributed_clock: VectorClock::new([(1, 0), (2, 0)]),
            hybrid_logical_timestamp: UnnamedHybridLogicalTimestamp::new(&FakeTimestamp(0_u32), 0),
        };
        let t2 = CompoundTimestamp {
            distributed_clock: VectorClock::default(),
            hybrid_logical_timestamp: UnnamedHybridLogicalTimestamp::new(&FakeTimestamp(0_u32), 0),
        };

        assert_eq!(
            t1, t2,
            "Non-existent vector clock entries should default to 0"
        );
    }

    #[test]
    fn hybrid_timestamp_concurrent_same_wall_clock() {
        let t1 = CompoundTimestamp {
            distributed_clock: VectorClock::new([(0, 1)]),
            hybrid_logical_timestamp: UnnamedHybridLogicalTimestamp::new(&FakeTimestamp(0_u32), 0),
        };
        let t2 = CompoundTimestamp {
            distributed_clock: VectorClock::new([(0, 1)]),
            hybrid_logical_timestamp: UnnamedHybridLogicalTimestamp::new(&FakeTimestamp(2_u32), 0),
        };

        assert_ne!(
            Some(Ordering::Equal),
            t1.partial_cmp(&t2),
            "Hybrid timestamps with different HLCs should never be equal"
        );
    }

    #[test]
    fn test_incomparable_timestamp() {
        let t1 = CompoundTimestamp {
            distributed_clock: VectorClock::<u16>::default(),
            hybrid_logical_timestamp: UnnamedHybridLogicalTimestamp::new(&FakeTimestamp(2_u32), 0),
        };
        let t2 = CompoundTimestamp {
            distributed_clock: VectorClock::default(),
            hybrid_logical_timestamp: UnnamedHybridLogicalTimestamp::new(&FakeTimestamp(1_u32), 0),
        };
        assert!(!CompoundTimestamp::is_comparable(&t1, &t2));
    }

    #[test]
    fn test_timestamp_comparison() {
        let t1 = CompoundTimestamp {
            distributed_clock: VectorClock::new([(1, 5), (2, 2)]),
            hybrid_logical_timestamp: UnnamedHybridLogicalTimestamp::new(&FakeTimestamp(10_u32), 0),
        };
        let t2 = CompoundTimestamp {
            distributed_clock: VectorClock::new([(1, 4), (2, 3)]),
            hybrid_logical_timestamp: UnnamedHybridLogicalTimestamp::new(&FakeTimestamp(30_u32), 0),
        };
        assert!(t1 < t2);
    }
}
