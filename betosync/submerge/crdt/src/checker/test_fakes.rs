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

//! A collection of test fakes that can be used in testing.
//!
//! Our CRDT implementations in this crate uses generic with relaxed trait bounds as much as
//! possible to enable substituting in these fakes for testing.

use arbitrary::Arbitrary;
use distributed_time::{fake_timestamp::FakeTimestamp, vector_clock::VectorClock};
use serde::{Deserialize, Serialize};

use crate::UpdateContext;

/// An enum with 3 states for testing. This is intended to be used as generic arguments to reduce
/// the state space while performing property-based tests.
#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    arbitrary::Arbitrary,
    Serialize,
    Deserialize,
)]
#[allow(missing_docs)]
pub enum TriState {
    #[default]
    A,
    B,
    C,
}

/// An [`UpdateContext`] implemented used for testing using [`FakeTimestamp`].
#[derive(Debug, Clone, Arbitrary)]
pub struct FakeContext<N: Ord, T: Into<u64> + TryFrom<u64> + Clone + Ord = u8> {
    /// The updater's node ID.
    pub updater: N,
    /// The fake timestamp provider.
    pub timestamp_provider: FakeTimestamp<T>,
    /// The version of the document.
    ///
    /// See [`UpdateContext::version`].
    pub version: VectorClock<N>,
}

impl<N: Ord, U: Into<u64> + TryFrom<u64> + Clone + Ord> FakeContext<N, U> {
    /// Create a new fake context with the given updater and timestamp.
    pub fn new(updater: N, timestamp: U) -> Self {
        Self {
            updater,
            timestamp_provider: FakeTimestamp(timestamp),
            version: VectorClock::default(),
        }
    }

    /// Create a new fake context with the given updater, timestamp, and version.
    pub fn new_with_version(updater: N, timestamp: U, version: VectorClock<N>) -> Self {
        Self {
            updater,
            timestamp_provider: FakeTimestamp(timestamp),
            version,
        }
    }
}

impl<N: Ord, U: Into<u64> + TryFrom<u64> + Clone + Ord> UpdateContext<N> for FakeContext<N, U> {
    type WallClock = FakeTimestamp<U>;

    fn updater(&self) -> &N {
        &self.updater
    }

    fn timestamp_provider(&self) -> &Self::WallClock {
        &self.timestamp_provider
    }

    fn version(&self) -> &VectorClock<N> {
        &self.version
    }
}
