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

//! Test that after incrementing an HLC, it is always greater than before increment.

#![cfg_attr(fuzzing, no_main)]
#![allow(missing_docs)]

use distributed_time::{
    fake_timestamp::FakeTimestamp, hybrid_logical_clock::HybridLogicalTimestamp, TotalTimestamp,
};

#[derive_fuzztest::fuzztest]
fn hlc_increment(
    timestamp: HybridLogicalTimestamp<u8>,
    updater: u8,
    wall_clock_time: FakeTimestamp,
) {
    #[allow(clippy::eq_op)]
    {
        // This might seem tautological, but it verifies that our `PartialEq` implementation works
        // as expected.
        assert_eq!(timestamp, timestamp);
    }
    if let Ok(incremented) = timestamp.increment(&updater, &wall_clock_time) {
        assert!(incremented > timestamp);
        assert!(timestamp < incremented);
    }
}
