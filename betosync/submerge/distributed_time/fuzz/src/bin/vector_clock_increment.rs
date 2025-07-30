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

//! Tests that a vector clock is greater after being incremented.

#![cfg_attr(fuzzing, no_main)]
#![allow(missing_docs)]

use std::cmp::Ordering;

use distributed_time::{vector_clock::VectorClock, DistributedClock as _};

#[derive_fuzztest::fuzztest]
fn vector_clock_increment(clock: VectorClock<u16>, id: u16) {
    if let Ok(incremented) = clock.increment(&id) {
        assert_eq!(Some(Ordering::Greater), incremented.partial_cmp(&clock));
    }
}
