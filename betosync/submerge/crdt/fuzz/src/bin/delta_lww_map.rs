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

//! Test that delta operations on a map behaves the same as a non-delta map.

#![cfg_attr(fuzzing, no_main)]
#![allow(missing_docs)]

use arbitrary::Arbitrary;
use crdt::{
    checker::{
        test_crdt_union::TestCrdtUnion,
        test_fakes::{FakeContext, TriState},
    },
    delta::DeltaOwned,
    lww_map::{LwwMap, LwwMapRead, LwwMapWrite},
};
use distributed_time::{fake_timestamp::FakeTotalTimestamp, DistributedClock};

#[derive(Arbitrary, Clone, Debug)]
pub enum Op {
    Set {
        key: TriState,
        ctx: FakeContext<()>,
        value: TestCrdtUnion,
    },
    Remove {
        key: TriState,
        ctx: FakeContext<()>,
    },
}

#[derive_fuzztest::fuzztest]
fn delta_lww_map(
    original: LwwMap<TriState, TestCrdtUnion, FakeTotalTimestamp<()>, ()>,
    ops: Vec<Op>,
) {
    let mut expected = original.clone();
    let mut delta = DeltaOwned::new(&original);

    for op in ops {
        match op {
            Op::Set {
                key,
                mut ctx,
                value,
            } => {
                // The ctx version must be at least as new as the subtree_version, since it is
                // supposed to be monotonic. Updating here to make sure the arbitrarily generated
                // update context is new enough.
                ctx.version
                    .least_upper_bound_in_place(&expected.subtree_version_least_upper_bound());
                let expected_result = expected.set_value(&ctx, key, value.clone());
                let delta_result = delta.set_value(&ctx, key, value);
                assert_eq!(expected_result, delta_result);
            }
            Op::Remove { key, mut ctx } => {
                // The ctx version must be at least as new as the subtree_version, since it is
                // supposed to be monotonic. Updating here to make sure the arbitrarily generated
                // update context is new enough.
                ctx.version
                    .least_upper_bound_in_place(&expected.subtree_version_least_upper_bound());
                let expected_result = expected.remove_value(&ctx, key);
                let delta_result = delta.remove_value(&ctx, key);
                assert_eq!(expected_result, delta_result);
            }
        }
    }

    assert_eq!(
        expected
            .keys()
            .map(|k| (k, expected.get_value(k).map(|f| f.merge())))
            .collect::<Vec<_>>(),
        delta
            .keys()
            .map(|k| (k, delta.get_value(k).map(|f| f.merge())))
            .collect::<Vec<_>>()
    );
    assert_eq!(expected, delta.merge().expect("merge must not fail"));
}
