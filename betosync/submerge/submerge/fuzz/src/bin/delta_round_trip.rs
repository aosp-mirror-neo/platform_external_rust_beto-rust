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

#![cfg_attr(fuzzing, no_main)]

use submerge::Document;
use submerge_fuzz::TestDataStoreConfig;

#[derive_fuzztest::fuzztest]
fn delta_round_trip(doc: Document<TestDataStoreConfig>) {
    let roundtrip = doc
        .clone()
        .into_full_delta()
        .into_document(&Document::default());
    assert_eq!(doc, roundtrip.unwrap());
}
