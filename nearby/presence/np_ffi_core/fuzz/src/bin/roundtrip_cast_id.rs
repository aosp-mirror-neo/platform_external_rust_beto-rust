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

//! Tests that round-trip ser->deser of Cast Ids is the identity.

#![cfg_attr(fuzzing, no_main)]
#![allow(missing_docs)]

use np_ffi_core::{deserialize::v1::*, v1::*};

#[derive_fuzztest::fuzztest]
fn roundtrip_cast_id(cast_id: CastId) {
    let de = cast_id.serialize();
    assert_eq!(
        de.try_deserialize(),
        Ok(V1DataElementDeserializationResult::Success(V1DataElement::CastId(cast_id)))
    );
}
