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

//! Tests that deserialization of arbitrary V1 DEs never panics.

#![cfg_attr(fuzzing, no_main)]
#![allow(missing_docs)]

use np_ffi_core::v1::GenericV1DataElement;

#[derive_fuzztest::fuzztest]
fn deserialization_does_not_panic(de: GenericV1DataElement) {
    let _ = de.try_deserialize();
}
