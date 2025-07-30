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

//! Tests that the timestamp comparison implementation forms a total order, as required by [`Ord`].

#![cfg_attr(fuzzing, no_main)]
#![allow(missing_docs)]

use std::cmp::Ordering;

use arbitrary::Arbitrary;
use distributed_time::{compound_timestamp::CompoundTimestamp, vector_clock::VectorClock};

#[derive(Debug, Arbitrary, Clone)]
enum TestCase {
    Reflexivity(CompoundTimestamp<VectorClock<u8>>),
    Antisymmetry([CompoundTimestamp<VectorClock<u8>>; 2]),
    Transitivity([CompoundTimestamp<VectorClock<u8>>; 3]),
}

#[derive_fuzztest::fuzztest]
fn compound_timestamp_total_order(t: TestCase) {
    match t {
        TestCase::Reflexivity(t) => {
            assert_eq!(Ordering::Equal, t.cmp(&t));
            assert!(t.eq(&t));
        }
        TestCase::Antisymmetry(t) => {
            assert!(match t[0].cmp(&t[1]) {
                Ordering::Less => t[1] > t[0],
                Ordering::Equal => t[1] == t[0],
                Ordering::Greater => t[1] < t[0],
            });
        }
        TestCase::Transitivity(t) => {
            let max = (&t[0]).max(&t[1]).max(&t[2]);
            assert!(max >= &t[0], "max={max:?} t[0]={:?}", t[0]);
            assert!(max >= &t[1], "max={max:?} t[1]={:?}", t[1]);
            assert!(max >= &t[2], "max={max:?} t[2]={:?}", t[2]);

            let min = (&t[0]).min(&t[1]).min(&t[2]);
            assert!(min <= &t[0], "min={min:?} t[0]={:?}", t[0]);
            assert!(min <= &t[1], "min={min:?} t[1]={:?}", t[1]);
            assert!(min <= &t[2], "min={min:?} t[2]={:?}", t[2]);
        }
    }
}
