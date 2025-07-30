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

//! A fake timestamp suitable for use in testing.
//!
//! See [`FakeTimestamp`].

use arbitrary::Arbitrary;
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, marker::PhantomData};

use crate::{TimestampOverflow, TotalTimestamp, WallTimestampProvider};

/// A fake timestamp suitable for use in testing.
///
/// This timestamp contains the numeric timestamp value and can be used as the
/// [`WallTimestampProvider`] type during testing.
///
/// This fake timestamp is generic over the underlying type so that the search space can be
/// configured to match the needs of a test when used with [`Arbitrary`].
#[derive(
    Clone, Default, Debug, Arbitrary, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash,
)]
pub struct FakeTimestamp<U: Into<u64> + TryFrom<u64> + Clone + Ord = u8>(pub U);

impl<U: Into<u64> + TryFrom<u64> + Clone + Ord> WallTimestampProvider for FakeTimestamp<U> {
    fn now(&self) -> u64 {
        self.0.clone().into()
    }
}

/// A fake [`TotalTimestamp`] that is backed by a `u8`.
#[derive(Clone, Default, Debug, Arbitrary, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FakeTotalTimestamp<N> {
    timestamp: u8,
    _marker: PhantomData<N>,
}

impl<N> FakeTotalTimestamp<N> {
    /// Create a new instance from the given timestamp.
    pub fn new(timestamp: u8) -> Self {
        Self {
            timestamp,
            _marker: PhantomData,
        }
    }
}

#[allow(clippy::panic)]
impl<N: Ord> TotalTimestamp for FakeTotalTimestamp<N> {
    type NodeId = N;

    fn increment(
        &self,
        _updater: &N,
        _timestamp_provider: &impl WallTimestampProvider,
    ) -> Result<Self, TimestampOverflow>
    where
        Self: Sized,
    {
        Ok(Self {
            timestamp: self.timestamp.checked_add(1).ok_or(TimestampOverflow)?,
            _marker: PhantomData,
        })
    }

    fn new_with_context(_updater: &N, timestamp_provider: &impl WallTimestampProvider) -> Self {
        Self {
            timestamp: timestamp_provider
                .now()
                .try_into()
                .unwrap_or_else(|_| panic!()),
            _marker: PhantomData,
        }
    }
}
