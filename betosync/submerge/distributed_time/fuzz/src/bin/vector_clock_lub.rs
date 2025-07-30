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

//! Tests that [`VectorClock::least_upper_bound`] indeed returns a least upper bound.

#![cfg_attr(fuzzing, no_main)]
#![allow(missing_docs)]
#![allow(clippy::indexing_slicing)]

use distributed_time::{vector_clock::VectorClock, DistributedClock};

#[derive_fuzztest::fuzztest]
fn vectror_clock_lub(a: VectorClock<u16>, b: VectorClock<u16>) {
    let lub = VectorClock::least_upper_bound(&a, &b);
    assert!(lub >= a);
    assert!(lub >= b);
    let a_dots = a.into_dots();
    let b_dots = b.into_dots();
    for (node, lub_value) in lub.into_dots() {
        assert!(Some(&lub_value) == a_dots.get(&node) || Some(&lub_value) == b_dots.get(&node));
    }
}
