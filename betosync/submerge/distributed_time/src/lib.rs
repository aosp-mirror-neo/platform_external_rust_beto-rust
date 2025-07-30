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

//! Traits and type implementations for time-tracking in distributed systems.
//!
//! See these provided implementations:
//! - [`VectorClock`][`vector_clock::VectorClock`]
//! - [`HybridLogicalTimestamp`][hybrid_logical_clock::HybridLogicalTimestamp]
//! - [`CompoundTimestamp`][compound_timestamp::CompoundTimestamp]
//!
//! And traits:
//! - [`TotalTimestamp`]
//! - [`DistributedClock`]
//! - [`WallTimestampProvider`]

pub mod compound_timestamp;
pub mod fake_timestamp;
pub mod hybrid_logical_clock;
pub mod vector_clock;

use arbitrary::Arbitrary;
use std::{
    cmp::Ordering,
    fmt::Debug,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

/// An error returned when the operation will otherwise cause the timestamp value to overflow.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("TimestampOverflow")]
pub struct TimestampOverflow;

/// A total-ordered timestamp.
///
/// This timestamp must implement `Ord` and follow the requirements there. In particular, make sure
/// that there cannot be any cycles where `a < b < c < a`.
pub trait TotalTimestamp: Ord {
    /// The node ID type. Can be `()` if the timestamp does not need to track metadata per-node.
    type NodeId;

    /// Create a new timestamp with the given context.
    ///
    /// The physical component of the timestamp comes from `time_context`, whereas the logical
    /// component will be at its initial "bottom" state.
    fn new_with_context(
        updater: &Self::NodeId,
        timestamp_provider: &impl WallTimestampProvider,
    ) -> Self;

    /// Increment the timestamp.
    ///
    /// The incremented timestamp must be larger than `self`.
    fn increment(
        &self,
        updater: &Self::NodeId,
        timestamp_provider: &impl WallTimestampProvider,
    ) -> Result<Self, TimestampOverflow>
    where
        Self: Sized;

    /// Increment the given timestamp if it is `Some`, or create a new one with the given context.
    fn increment_or_new(
        timestamp: Option<&Self>,
        updater: &Self::NodeId,
        timestamp_provider: &impl WallTimestampProvider,
    ) -> Result<Self, TimestampOverflow>
    where
        Self: Sized,
    {
        match timestamp {
            Some(t) => t.increment(updater, timestamp_provider),
            None => Ok(Self::new_with_context(updater, timestamp_provider)),
        }
    }
}

/// A distributed clock that tracks causality, like vector clock.
///
/// The [`PartialOrd`] implementations must obey the happens-before causality order, and return
/// `None` if the events are concurrent.
pub trait DistributedClock: PartialOrd + Eq {
    /// The node ID type. Can be `()` if the timestamp does not need to track metadata per-node.
    type NodeId;

    /// Increment the timestamp.
    ///
    /// The incremented timestamp must be larger than `self`.
    fn increment(&self, ctx: &Self::NodeId) -> Result<Self, TimestampOverflow>
    where
        Self: Sized;

    /// Calculate the least upper bound of the given timestamps and update `self` in-place.
    ///
    /// A resulting timestamp `T` must satisfy `self' >= self` and `self' >= other`.
    fn least_upper_bound_in_place(&mut self, other: &Self);

    /// Calculate the least upper bound of the given timestamps.
    ///
    /// A resulting timestamp `T` must satisfy `T >= a` and `T >= b`.
    fn least_upper_bound(a: &Self, b: &Self) -> Self
    where
        Self: Clone + Sized,
    {
        let mut new_timestamp = a.clone();
        new_timestamp.least_upper_bound_in_place(b);
        new_timestamp
    }
}

/// A provider for the current wall clock time.
///
/// This wall-clock is typically used as a tie-breaker for concurrent changes in last-writer-wins
/// scenarios. The clock may go backwards in time (not required to be monotonic).
///
/// When used in a [`HybridLogicalTimestamp`][hybrid_logical_clock::HybridLogicalTimestamp], a
/// skewed timestamp may unfairly bias concurrent changes to a particular node, and may affect
/// future updates' ability to effectively tie-break using the timestamp, but it will not affect
/// data consistency. (In the worst case, a heavily skewed clock will cause the timestamps to behave
/// as if they are Lamport timestamps.)
pub trait WallTimestampProvider: Clone {
    /// Returns the number of milliseconds since Unix epoch.
    fn now(&self) -> u64;
}

/// An implementation of [`WallTimestampProvider`] that is implemented using `std` APIs.
#[derive(Clone, Debug, Default, Arbitrary)]
pub struct SystemTimeProvider;

impl WallTimestampProvider for SystemTimeProvider {
    fn now(&self) -> u64 {
        let epoch_millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_millis();
        // Converting from u128 to u64 overflowed. Saturate to u64::MAX
        epoch_millis.try_into().unwrap_or(u64::MAX)
    }
}

/// Same as [`Ord`], but the order may not have a semantic meaning.
///
/// Implementations can be used to obtain a total order in cases that just needs an
/// arbitrary-but-consistent order, like in [`std::collections::BTreeSet`] or
/// [`std::collections::BTreeMap`].
///
/// Unlike [`Ord`], types that implement both [`NonSemanticOrd`] and [`PartialOrd`] are allowed to
/// return `None` from [`PartialOrd::partial_cmp`]. If `partial_cmp` returns a value that's not
/// `None`, `non_semantic_cmp` must return the same value.
pub trait NonSemanticOrd: Eq {
    /// This method returns an [`Ordering`] between `self` and `other`.
    fn non_semantic_cmp(&self, other: &Self) -> Ordering;
}

impl<T: Ord> NonSemanticOrd for T {
    fn non_semantic_cmp(&self, other: &Self) -> Ordering {
        self.cmp(other)
    }
}
