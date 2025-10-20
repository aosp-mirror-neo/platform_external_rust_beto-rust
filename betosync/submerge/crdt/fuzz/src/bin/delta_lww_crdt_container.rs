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

//! Test that delta operations on a container behaves the same as a non-delta container.

#![cfg_attr(fuzzing, no_main)]
#![allow(missing_docs)]

use arbitrary::Arbitrary;
use crdt::{
    checker::{test_crdt_union::TestCrdtUnion, test_fakes::FakeContext},
    delta::DeltaOwned,
    lww_crdt_container::{LwwCrdtContainer, LwwCrdtContainerRead, LwwCrdtContainerWrite},
};
use distributed_time::{fake_timestamp::FakeTotalTimestamp, DistributedClock};

#[derive(Arbitrary, Clone, Debug)]
pub enum Op {
    Set {
        ctx: FakeContext<()>,
        value: TestCrdtUnion,
    },
    Remove {
        ctx: FakeContext<()>,
    },
}

#[derive_fuzztest::fuzztest]
fn delta_lww_crdt_container(
    original: LwwCrdtContainer<TestCrdtUnion, FakeTotalTimestamp<()>, ()>,
    ops: Vec<Op>,
) {
    let mut expected = original.clone();
    let mut delta = DeltaOwned::new(&original);

    for op in ops {
        match op {
            Op::Set { mut ctx, value } => {
                // The ctx version must be at least as new as the subtree_version, since it is
                // supposed to be monotonic. Updating here to make sure the arbitrarily generated
                // update context is new enough.
                ctx.version
                    .least_upper_bound_in_place(expected.subtree_version());
                let expected_result = expected.set_value(&ctx, value.clone());
                let delta_result = delta.set_value(&ctx, value);

                assert_eq!(expected_result, delta_result);
            }
            Op::Remove { mut ctx } => {
                // The ctx version must be at least as new as the subtree_version, since it is
                // supposed to be monotonic. Updating here to make sure the arbitrarily generated
                // update context is new enough.
                ctx.version
                    .least_upper_bound_in_place(expected.subtree_version());
                let expected_result = expected.remove_value(&ctx);
                let delta_result = delta.remove_value(&ctx);

                assert_eq!(expected_result, delta_result);
            }
        }
    }

    assert_eq!(
        expected.get_value().map(|d| d.merge()),
        delta.get_value().map(|d| d.merge())
    );
    let ctx = FakeContext::new_with_version((), 0_u8, expected.subtree_version().clone());
    assert_eq!(
        expected
            .get_value_mut(&ctx)
            .map(|result| result.map(|d| d.merge())),
        delta
            .get_value_mut(&ctx)
            .map(|result| result.map(|d| d.merge())),
    );
    assert_eq!(expected, delta.merge().expect("merge must not fail"));
}
