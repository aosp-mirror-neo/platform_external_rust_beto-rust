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

//! Test that the `Default` implementation of the CRDTs all return bottom states.
//!
//! A _bottom state_ of a CRDT refers to a state that, when merged with any other valid state `S`,
//! will result in `S`. It's called "bottom state" because in a Hasse diagram of CRDT states this
//! state will show up in the bottom.

#![cfg_attr(fuzzing, no_main)]

use crdt::{
    checker::{test_crdt_union::TestCrdtUnion, test_fakes::TriState, utils::DeterministicHasher},
    lww_crdt_container::LwwCrdtContainer,
    lww_map::LwwMap,
    register::Register,
    set::Set,
    vector_data::VectorData,
    CrdtState,
};
use distributed_time::{fake_timestamp::FakeTotalTimestamp, vector_clock::VectorClock};

#[derive(arbitrary::Arbitrary, Clone, Debug)]
enum ArbitraryCrdt {
    Set(Set<TriState, DeterministicHasher>),
    Register(Register<TriState, VectorClock<u8>>),
    VectorData(VectorData<TriState, TriState>),
    Map(LwwMap<TriState, TestCrdtUnion, FakeTotalTimestamp<u8>, u8>),
    Container(LwwCrdtContainer<TestCrdtUnion, FakeTotalTimestamp<u8>, u8>),
}

#[derive_fuzztest::fuzztest]
fn default_bottom_state(crdt: ArbitraryCrdt) {
    match crdt {
        ArbitraryCrdt::Set(original) => {
            assert_eq!(
                original,
                CrdtState::merge(&original, &Default::default()).expect("merge must not fail")
            );
        }
        ArbitraryCrdt::Register(original) => {
            assert_eq!(
                original,
                CrdtState::merge(&original, &Default::default()).expect("merge must not fail")
            );
        }
        ArbitraryCrdt::VectorData(original) => {
            assert_eq!(
                original,
                CrdtState::merge(&original, &Default::default()).expect("merge must not fail")
            );
        }
        ArbitraryCrdt::Map(original) => {
            assert_eq!(
                original,
                CrdtState::merge(&original, &Default::default()).expect("merge must not fail")
            );
        }
        ArbitraryCrdt::Container(original) => {
            assert_eq!(
                original,
                CrdtState::merge(&original, &Default::default()).expect("merge must not fail")
            );
        }
    }
}
