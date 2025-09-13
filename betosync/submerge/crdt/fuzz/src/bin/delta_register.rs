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

//! Test that delta operations on a register behaves the same as a non-delta register.

#![cfg_attr(fuzzing, no_main)]
#![allow(missing_docs)]

use crdt::{
    checker::{
        simulation::{Operation, SimulationContext},
        test_fakes::TriState,
    },
    delta::{AsDeltaMut, DeltaOwned},
    register::{checker::RegOp, Register, RegisterRead},
};
use distributed_time::vector_clock::VectorClock;

#[derive_fuzztest::fuzztest]
fn delta_register(
    original: Register<TriState, VectorClock<u8>>,
    ops: Vec<RegOp<u8, TriState>>,
    ctx: SimulationContext<u8>,
) {
    let mut expected = original.clone();
    let mut delta = DeltaOwned::new(&original);

    for op in ops {
        op.clone().apply(expected.as_mut(), &ctx);
        op.apply(delta.as_mut(), &ctx);
    }

    assert_eq!(expected.get_all(), delta.get_all());
    assert_eq!(expected.get(), delta.get());
    assert_eq!(expected, delta.merge().expect("merge must not fail"));
}
