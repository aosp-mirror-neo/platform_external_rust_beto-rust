// Copyright 2024 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Credentials should not be able to be added to a credential slab when the `nocrypto` feature is
//! in use.

use derive_fuzztest;
use handle_map::OwnedHandle;
use np_ffi_core::credentials::{
    create_credential_slab, AddV0CredentialToSlabResult, AddV1CredentialToSlabResult,
    CreateCredentialSlabResult, CredentialSlab, MatchedCredential, V0BroadcastCredential,
    V0DiscoveryCredential, V1BroadcastCredential, V1DiscoveryCredential,
};
use np_ffi_core::serialize::v1::{
    create_v1_advertisement_builder, CreateV1AdvertisementBuilderResult,
    CreateV1SectionBuilderResult,
};
use np_ffi_core::v1::V1VerificationMode;

fn make_slab() -> OwnedHandle<CredentialSlab> {
    let CreateCredentialSlabResult::Success(slab) = create_credential_slab() else {
        panic!("failed to create slab");
    };
    OwnedHandle::new(slab)
}

#[derive_fuzztest::proptest]
fn add_v0_credential_to_slab(cred: V0DiscoveryCredential, cred_id: i64) {
    let slab = make_slab();
    let match_data = MatchedCredential::new(cred_id, &[], [0u8; 32].into());

    let res = slab.add_v0(cred, match_data);

    assert!(matches!(res, AddV0CredentialToSlabResult::InvalidHandle));
}

#[derive_fuzztest::proptest]
fn add_v1_credential_to_slab(cred: V1DiscoveryCredential, cred_id: i64) {
    let slab = make_slab();
    let match_data = MatchedCredential::new(cred_id, &[], [0u8; 32].into());

    let res = slab.add_v1(cred, match_data);

    assert!(matches!(res, AddV1CredentialToSlabResult::InvalidHandle));
}

#[derive_fuzztest::proptest]
fn create_encrypted_v0_builder(broadcast_cred: V0BroadcastCredential, salt: [u8; 2]) {
    use np_ffi_core::serialize::v0::{
        create_v0_encrypted_advertisement_builder, CreateV0AdvertisementBuilderResult,
    };

    let res = create_v0_encrypted_advertisement_builder(broadcast_cred, salt.into());
    assert!(matches!(res, CreateV0AdvertisementBuilderResult::NoSpaceLeft));
}

#[derive_fuzztest::proptest]
fn create_encrypted_v1_section_builder(
    broadcast_cred: V1BroadcastCredential,
    verification_mode: V1VerificationMode,
) {
    let CreateV1AdvertisementBuilderResult::Success(builder) = create_v1_advertisement_builder()
    else {
        panic!("failed to create public builder");
    };
    let builder = OwnedHandle::new(builder);

    let res = builder.encrypted_section_builder(broadcast_cred, verification_mode);

    assert!(matches!(res, CreateV1SectionBuilderResult::NoSpaceLeft));
}
