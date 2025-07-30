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

//! Tests that any increment / least-upper-bound paths won't generate two events with the same
//! timestamp.

#![cfg_attr(fuzzing, no_main)]
#![allow(missing_docs)]

use std::collections::{BTreeMap, BTreeSet};

use arbitrary::Arbitrary;
use distributed_time::{
    fake_timestamp::FakeTimestamp, hybrid_logical_clock::HybridLogicalTimestamp, TotalTimestamp,
};

#[derive(Debug, Clone, Arbitrary)]
enum Op {
    Increment {
        updater: u8,
        timestamp: FakeTimestamp,
    },
}

#[derive_fuzztest::fuzztest]
fn hlc_uniqueness(starting_timestamp: HybridLogicalTimestamp<u8>, operations: Vec<Op>) {
    let mut seen_timestamps = BTreeSet::new();
    let _ = seen_timestamps.insert(starting_timestamp.clone());
    let mut map = BTreeMap::new();
    let _ = map.insert(0_u8, starting_timestamp.clone());
    for op in operations {
        match op {
            Op::Increment { updater, timestamp } => {
                let entry = map
                    .entry(updater)
                    .or_insert_with(|| starting_timestamp.clone());
                if let Ok(new_timestamp) = entry.increment(&updater, &timestamp) {
                    assert!(seen_timestamps.insert(new_timestamp.clone()));
                    *entry = new_timestamp;
                };
            }
        }
    }
}
