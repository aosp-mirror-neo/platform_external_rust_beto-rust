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

use std::collections::HashSet;

use crate::{
    checker::{utils::DeterministicHasher, MergeInvariantTest},
    CrdtState,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct SetUnion(HashSet<u8, DeterministicHasher>);

impl<'a> arbitrary::Arbitrary<'a> for SetUnion {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(SetUnion(
            u.arbitrary_iter()?.collect::<Result<HashSet<_, _>, _>>()?,
        ))
    }
}

impl CrdtState for SetUnion {
    fn merge(a: &Self, b: &Self) -> Self {
        SetUnion(a.0.union(&b.0).cloned().collect())
    }
}

#[derive_fuzztest::proptest]
fn merge_invariant_test(input: MergeInvariantTest<SetUnion>) {
    input.test()
}
