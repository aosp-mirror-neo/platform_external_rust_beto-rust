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

//! Tests that attempting to deserialize with `nocrypto` enabled doesn't panic.

#![cfg_attr(fuzzing, no_main)]
#![allow(missing_docs)]

use handle_map::OwnedHandle;
use np_ffi_core::{
    credentials::{
        create_credential_book_from_slab, create_credential_slab, CreateCredentialBookResult,
        CreateCredentialSlabResult, CredentialBook,
    },
    deserialize::deserialize_advertisement_from_slice,
};

fn make_empty_credential_book() -> OwnedHandle<CredentialBook> {
    let CreateCredentialSlabResult::Success(slab) = create_credential_slab() else {
        panic!("Failed to create credential slab");
    };
    let CreateCredentialBookResult::Success(book) = create_credential_book_from_slab(slab) else {
        panic!("Failed to create credential book");
    };
    OwnedHandle::new(book)
}

#[derive_fuzztest::fuzztest]
fn deserialize_random_advertisement(adv: Vec<u8>) {
    let book = make_empty_credential_book();
    let _ = deserialize_advertisement_from_slice(&adv, *book).deallocate();
}
