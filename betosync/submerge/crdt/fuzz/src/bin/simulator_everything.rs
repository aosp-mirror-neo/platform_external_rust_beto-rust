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

//! Simulator that runs through all possible operations in a nested CRDT tree implemented by this
//! crate.

#![cfg_attr(fuzzing, no_main)]
#![allow(missing_docs)]

use std::collections::BTreeMap;

use arbitrary::Arbitrary;
use crdt::{
    checker::{
        simulation::{Operation, Simulation, SimulationContext},
        test_fakes::TriState,
        utils::DeterministicHasher,
    },
    delta::DeltaMut,
    lww_map::{LwwMap, LwwMapRead, LwwMapWrite},
    register::{checker::RegOp, Register},
    set::{checker::SetOp, Set},
    typed_crdt::{PlainTypes, TypedCrdt},
    vector_data::{checker::VecDataOp, VectorData},
};
use distributed_time::hybrid_logical_clock::HybridLogicalTimestamp;

type Node = TriState;

#[derive(Debug, Clone, Arbitrary)]
pub enum CrdtType {
    Map,
    Register,
    Set,
    VectorData,
    None,
}

/// Map operation that may be applied to the root or to one of the child if its type is a Map.
#[derive(Debug, Clone, Arbitrary)]
pub enum MapOperation {
    SetType {
        node: Node,
        key: String,
        value: CrdtType,
    },
    ChildOperation {
        node: Node,
        key: String,
        op: ChildOperation,
    },
    ApplyChanges {
        node: Node,
        plain_type: BTreeMap<String, PlainTypes<Node, TriState, DeterministicHasher>>,
    },
}

#[derive(Debug, Clone, Arbitrary)]
pub enum ChildOperation {
    Map(Box<MapOperation>),
    RegisterOp(RegOp<Node, TriState>),
    SetOp(SetOp<TriState>),
    VectorDataOp(VecDataOp<Node, TriState>),
}

type TestLwwMap = LwwMap<
    String,
    TypedCrdt<TriState, Node, DeterministicHasher>,
    HybridLogicalTimestamp<Node>,
    Node,
>;

impl Operation<TestLwwMap, Node> for MapOperation {
    fn apply(self, mut map: DeltaMut<TestLwwMap>, sim_ctx: &SimulationContext<Node>) {
        match self {
            MapOperation::SetType { node, key, value } => {
                let ctx = sim_ctx.context(&node);
                match value {
                    CrdtType::Map => {
                        let _ = map.get_child_or_default::<TestLwwMap>(&ctx, key);
                    }
                    CrdtType::Register => {
                        let _ = map.get_child_or_default::<Register<_, _>>(&ctx, key);
                    }
                    CrdtType::None => {
                        let _ = map.remove_value(&ctx, key);
                    }
                    CrdtType::Set => {
                        let _ = map.get_child_or_default::<Set<_, _>>(&ctx, key);
                    }
                    CrdtType::VectorData => {
                        let _ = map.get_child_or_default::<VectorData<_, _>>(&ctx, key);
                    }
                }
            }
            MapOperation::ChildOperation { node, key, op } => {
                let ctx = sim_ctx.context(&node);
                match op {
                    ChildOperation::Map(map_op) => {
                        if let Ok(Some(child_map)) =
                            map.get_child_mut::<LwwMap<_, _, _, _>>(&ctx, key)
                        {
                            map_op.apply(child_map, sim_ctx);
                        }
                    }
                    ChildOperation::RegisterOp(reg_op) => {
                        if let Ok(Some(reg)) = map.get_child_mut::<Register<_, _>>(&ctx, key) {
                            reg_op.apply(reg, sim_ctx);
                        }
                    }
                    ChildOperation::SetOp(set_op) => {
                        if let Ok(Some(set)) = map.get_child_mut::<Set<_, _>>(&ctx, key) {
                            set_op.apply(set, sim_ctx);
                        }
                    }
                    ChildOperation::VectorDataOp(vec_data_op) => {
                        if let Ok(Some(vd)) = map.get_child_mut::<VectorData<_, _>>(&ctx, key) {
                            vec_data_op.apply(vd, sim_ctx);
                        }
                    }
                }
            }
            MapOperation::ApplyChanges { node, plain_type } => {
                if map
                    .apply_changes(&sim_ctx.context(&node), plain_type.clone())
                    .is_ok()
                {
                    assert_eq!(plain_type, map.to_plain());
                }
            }
        }
    }
}

#[derive_fuzztest::fuzztest]
fn simulator_lwwmap(sim: Simulation<TestLwwMap, MapOperation>) {
    let _ = sim.run();
}
