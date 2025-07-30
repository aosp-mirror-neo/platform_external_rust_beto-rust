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

//! Test that the merge invariants are obeyed by a register.
//!
//! See [`MergeInvariantTest`]

#![cfg_attr(fuzzing, no_main)]

use crdt::{
    checker::{test_fakes::TriState, MergeInvariantTest},
    register::Register,
};
use distributed_time::vector_clock::VectorClock;

#[derive_fuzztest::fuzztest]
fn merge_register(test: MergeInvariantTest<Register<TriState, VectorClock<u8>>>) {
    test.test();
}
