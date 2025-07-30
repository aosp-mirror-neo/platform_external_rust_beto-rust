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

//! Simulator that runs through all possible operations in an LWW Map.

#![cfg_attr(fuzzing, no_main)]
#![allow(missing_docs, clippy::unwrap_used)]

use arbitrary::Arbitrary;
use crdt::{
    checker::{
        simulation::{Operation, Simulation, SimulationContext},
        test_crdt_union::{TestCrdtUnion, TestCrdtUnionMut, TestVariant1, TestVariant2},
        test_fakes::TriState,
    },
    delta::DeltaMut,
    lww_crdt_container::CrdtUnion,
    lww_map::{LwwMap, LwwMapRead, LwwMapWrite},
};
use distributed_time::hybrid_logical_clock::HybridLogicalTimestamp;

type Node = TriState;

#[derive(Debug, Clone, Arbitrary)]
pub enum MapOperation {
    SetValue {
        node: Node,
        key: TriState,
        value: TestCrdtUnion,
    },
    RemoveValue {
        node: Node,
        key: TriState,
    },
    SetChildValue {
        key: TriState,
        value: u8,
        node: Node,
    },
}

type State = LwwMap<TriState, TestCrdtUnion, HybridLogicalTimestamp<Node>, Node>;

impl Operation<State, TriState> for MapOperation {
    fn apply(self, mut map: DeltaMut<State>, context: &SimulationContext<TriState>) {
        match self {
            Self::SetValue { node, key, value } => {
                if map
                    .set_value(&context.context(&node), key, value.clone())
                    .is_ok()
                {
                    assert_eq!(
                        <TestCrdtUnion as CrdtUnion<Node>>::create_ref(None, Some(&value)),
                        map.get_value(&key)
                    )
                }
            }
            Self::RemoveValue { node, key } => {
                if map.remove_value(&context.context(&node), key).is_ok() {
                    assert_eq!(None, map.get_value(&key));
                }
            }
            Self::SetChildValue { key, value, node } => {
                match map.get_value_mut(&context.context(&node), key) {
                    Ok(Some(TestCrdtUnionMut::Type1(mut child1))) => {
                        child1.set_value(value);
                        assert_eq!(
                            value,
                            map.get_child::<TestVariant1>(&key).unwrap().get_value()
                        );
                        assert_eq!(None, map.get_child::<TestVariant2>(&key));
                    }
                    Ok(Some(TestCrdtUnionMut::Type2(mut child2))) => {
                        child2.set_value(value);
                        assert_eq!(
                            value,
                            map.get_child::<TestVariant2>(&key).unwrap().get_value()
                        );
                        assert_eq!(None, map.get_child::<TestVariant1>(&key));
                    }
                    Ok(None) => {
                        assert_eq!(None, map.get_value(&key));
                    }
                    Err(_) => {
                        // Ignore timestamp overflows.
                    }
                }
            }
        }
    }
}

#[derive_fuzztest::fuzztest]
fn simulator_lwwmap(sim: Simulation<State, MapOperation>) {
    let _ = sim.run();
}
