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

//! Tests that the `Ord`, `PartialOrd`, and `PartialEq` implementations are consistent with each
//! other, as required in their documentation.

#![cfg_attr(fuzzing, no_main)]

use std::cmp::Ordering;

use distributed_time::hybrid_logical_clock::HybridLogicalTimestamp;

#[derive_fuzztest::fuzztest]
fn hlc_relation_consistency(
    timestamp1: HybridLogicalTimestamp<u8>,
    timestamp2: HybridLogicalTimestamp<u8>,
) {
    match timestamp1.cmp(&timestamp2) {
        std::cmp::Ordering::Less => {
            assert_eq!(Some(Ordering::Less), timestamp1.partial_cmp(&timestamp2));
            assert!(!timestamp1.eq(&timestamp2))
        }
        std::cmp::Ordering::Equal => {
            assert_eq!(Some(Ordering::Equal), timestamp1.partial_cmp(&timestamp2));
            assert!(timestamp1.eq(&timestamp2))
        }
        std::cmp::Ordering::Greater => {
            assert_eq!(Some(Ordering::Greater), timestamp1.partial_cmp(&timestamp2));
            assert!(!timestamp1.eq(&timestamp2))
        }
    }
}
