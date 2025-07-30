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

//! Tests that round-trip ser->deser of the V1 actions DE is the identity when restricted to
//! serializable actions-sets.

#![cfg_attr(fuzzing, no_main)]
#![allow(missing_docs)]

use derive_fuzztest::TestResult;
use np_ffi_core::{deserialize::v1::*, serialize::v1::*, v1::*};

#[derive_fuzztest::fuzztest]
fn roundtrip_v1_actions(actions: V1ActionsBitmap) -> TestResult {
    let V1SerializeActionsBitmapResult::Success(de) = actions.try_serialize() else {
        // We've encountered either too many or too few actions to
        // serialize, so the rest of the test cannot procceed.
        // We're still ensuring that failed serialization doesn't panic.
        return TestResult::Discard;
    };
    assert_eq!(
        de.try_deserialize(),
        Ok(V1DataElementDeserializationResult::Success(V1DataElement::Actions(actions)))
    );
    TestResult::Passed
}
