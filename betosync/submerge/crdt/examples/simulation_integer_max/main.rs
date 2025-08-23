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

#![allow(missing_docs)]

use arbitrary::Arbitrary;
use crdt::{
    checker::{
        simulation::{Operation, Simulation, SimulationContext},
        test_fakes::TriState,
    },
    delta::{AsDeltaMut, AsDeltaRef, DeltaMut},
    CrdtState,
};

/// Our trivial example CRDT.
#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Arbitrary)]
pub struct IntegerMaxCrdt(u8);

/// Merges by taking the max of the two values.
impl CrdtState for IntegerMaxCrdt {
    fn merge(a: &Self, b: &Self) -> Self {
        a.max(b).clone()
    }
}

/// Define the operations that can be performed on the CRDT.
#[derive(Debug, Clone, Arbitrary)]
pub enum IntegerMaxOp {
    /// Only operation supported in this simulation is setting the integer value
    SetInteger { value: u8 },
    // Operations outside of the CRDT, like setting the wall clock can also be defined here.
}

impl Operation<IntegerMaxCrdt, TriState> for IntegerMaxOp {
    fn apply(self, mut state: DeltaMut<IntegerMaxCrdt>, _ctx: &SimulationContext<TriState>) {
        match self {
            IntegerMaxOp::SetInteger { value } => {
                if value > state.as_ref().merge().0 {
                    state.delta_mut().0 = value;
                }
            }
        }
    }
}

#[derive_fuzztest::fuzztest]
fn simulator(sim: Simulation<IntegerMaxCrdt, IntegerMaxOp>) {
    let _ = sim.run();
}
