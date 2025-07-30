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

//! Simulator that runs through all possible operations in a set.

#![cfg_attr(fuzzing, no_main)]
#![allow(missing_docs)]

use crdt::{
    checker::{simulation::Simulation, test_fakes::TriState, utils::DeterministicHasher},
    set::{checker::SetOp, Set},
};

#[derive_fuzztest::fuzztest]
fn simulator_set(sim: Simulation<Set<TriState, DeterministicHasher>, SetOp<TriState>>) {
    let _ = sim.run();
}
