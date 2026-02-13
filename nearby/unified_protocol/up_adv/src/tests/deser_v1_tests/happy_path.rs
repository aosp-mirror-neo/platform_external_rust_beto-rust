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

extern crate std;

use super::*;
use crate::buffer::{AdvBuffer, StackBuffer, VecBuffer};
use crate::{deserialization_arena, extended::salt::*};
use rand::SeedableRng;
use std::collections::HashSet;

const TEST_BUFFER_SIZE: usize = 250;

#[test]
fn deserialize_rand_identities_single_section_finds_correct_one_mic_derived_salt() {
    let mut rng = StdRng::from_entropy();

    for _ in 0..100 {
        let identities = TestIdentities::generate::<100, _>(&mut rng);
        let identity = identities.pick_random_identity(&mut rng);
        let book = identities.build_cred_book::<CryptoProviderImpl>();

        // Root Salt setup
        let root_salt = rng.gen::<[u8; 16]>();
        let section_salt =
            crate::extended::salt::derive_section_salt::<CryptoProviderImpl>(&root_salt, 0)
                .unwrap();
        let salt = MultiSalt::Derived(section_salt);

        let buffer = StackBuffer::<TEST_BUFFER_SIZE>::new();
        let mut adv_builder = AdvBuilder::<StackBuffer<TEST_BUFFER_SIZE>>::new(buffer);

        // Use 0x30 for MIC calculation so it matches the patched header
        let section_config = add_mic_with_salt_to_adv::<
            _,
            CryptoProviderImpl,
            0,
            StackBuffer<TEST_BUFFER_SIZE>,
        >(&mut rng, identity, &mut adv_builder, salt, 0x30)
        .unwrap();

        let adv = adv_builder.into_advertisement();

        // Patch header: 0x20 -> 0x30 + Root Salt
        let mut adv_bytes = adv.as_slice().to_vec();
        adv_bytes[0] = 0x30;
        let mut new_bytes = Vec::new();
        new_bytes.push(0x30);
        new_bytes.extend_from_slice(&root_salt);
        new_bytes.extend_from_slice(&adv_bytes[1..]);
        let adv_bytes = new_bytes;

        let mut buffer = StackBuffer::<TEST_BUFFER_SIZE>::new();
        let arena = deserialization_arena!(&mut buffer);
        let decrypted_contents = deser_v1::<_, _, CryptoProviderImpl>(arena, &adv_bytes, &book);
        assert_eq!(0, decrypted_contents.invalid_sections_count());
        let sections = decrypted_contents.into_sections();
        assert_eq!(1, sections.len());

        assert_section_equals(&section_config, &sections[0]);
    }
}

#[test]
fn v1_decrypted_mic_derived_salt_matches() {
    let mut rng = StdRng::from_entropy();
    // Must generate Root Salt and derive Section Salt
    let root_salt = rng.gen::<[u8; 16]>();
    let section_salt =
        crate::extended::salt::derive_section_salt::<CryptoProviderImpl>(&root_salt, 0).unwrap();
    let salt = MultiSalt::Derived(section_salt);

    let buffer = StackBuffer::<TEST_BUFFER_SIZE>::new();
    v1_decrypted_adv_salt_matches::<StackBuffer<TEST_BUFFER_SIZE>, _>(
        &mut rng,
        salt,
        Some(root_salt),
        |rng, identity, builder, salt| {
            add_mic_with_salt_to_adv::<_, CryptoProviderImpl, 0, StackBuffer<TEST_BUFFER_SIZE>>(
                rng, identity, builder, salt, 0x30,
            )
        },
        buffer,
    );
}

fn v1_decrypted_adv_salt_matches<
    B: AdvBuffer,
    F: for<'a> Fn(
        &mut StdRng,
        &'a TestIdentity,
        &mut AdvBuilder<B>,
        MultiSalt,
    ) -> Result<SectionConfig<'a>, AddSectionError>,
>(
    rng: &mut StdRng,
    salt: MultiSalt,
    root_salt: Option<[u8; 16]>,
    add_to_adv: F,
    buffer: B,
) {
    let identities = TestIdentities::generate::<1, _>(rng);
    let mut adv_builder = AdvBuilder::<B>::new(buffer);

    let _ = add_to_adv(rng, &identities.0[0], &mut adv_builder, salt);

    let adv = adv_builder.into_advertisement();

    // Patch header if needed
    let mut adv_bytes = adv.as_slice().to_vec();
    if let Some(rs) = root_salt {
        // 0x30 = V1 (0x20) | Root Salt (0x10)
        adv_bytes[0] = 0x30;
        // Insert root salt
        let mut new_bytes = Vec::new();
        new_bytes.push(0x30);
        new_bytes.extend_from_slice(&rs);
        new_bytes.extend_from_slice(&adv_bytes[1..]);
        adv_bytes = new_bytes;
    }

    let mut buffer = StackBuffer::<TEST_BUFFER_SIZE>::new();
    let arena = deserialization_arena!(&mut buffer);
    let cred_book = identities.build_cred_book::<CryptoProviderImpl>();

    let sections =
        deser_v1::<_, _, CryptoProviderImpl>(arena, &adv_bytes, &cred_book).into_sections();
    let section = &sections[0];
    let decrypted = match section {
        V1DeserializedSection::Plaintext(_) => {
            panic!("section is encrypted")
        }
        V1DeserializedSection::Decrypted(d) => d,
    };
    assert_eq!(salt, *decrypted.contents().salt())
}

#[test]
fn v1_decrypted_mic_derived_salt_multiple_sections_matches() {
    let mut rng = StdRng::from_entropy();
    let root_salt = rng.gen::<[u8; 16]>();

    let buffer = VecBuffer::new(1000);
    let mut adv_builder = AdvBuilder::<VecBuffer>::new(buffer);
    let mut salts = Vec::new();
    let mut identities = Vec::new();

    for i in 0..3 {
        let identity = TestIdentity::random(&mut rng);
        identities.push(identity.clone());

        let section_salt =
            crate::extended::salt::derive_section_salt::<CryptoProviderImpl>(&root_salt, i)
                .unwrap();
        salts.push(MultiSalt::Derived(section_salt));

        let _ = add_mic_with_salt_to_adv::<_, CryptoProviderImpl, 0, VecBuffer>(
            &mut rng,
            &identity,
            &mut adv_builder,
            MultiSalt::Derived(section_salt),
            0x30,
        )
        .unwrap();
    }

    let adv = adv_builder.into_advertisement();

    // Patch header
    let mut adv_bytes = adv.as_slice().to_vec();
    adv_bytes[0] = 0x30;
    let mut new_bytes = Vec::new();
    new_bytes.push(0x30);
    new_bytes.extend_from_slice(&root_salt);
    new_bytes.extend_from_slice(&adv_bytes[1..]);
    let adv_bytes = new_bytes;

    let mut buffer = VecBuffer::new(1000);
    let arena = deserialization_arena!(&mut buffer);
    let book = TestIdentities(identities).build_cred_book::<CryptoProviderImpl>();
    let contents = deser_v1::<_, _, CryptoProviderImpl>(arena, &adv_bytes, &book);
    assert_eq!(0, contents.invalid_sections_count());
    let sections = contents.into_sections();
    assert_eq!(3, sections.len());

    for (i, section) in sections.iter().enumerate() {
        let decrypted = match section {
            V1DeserializedSection::Plaintext(_) => panic!("expected encrypted"),
            V1DeserializedSection::Decrypted(d) => d,
        };
        assert_eq!(salts[i], *decrypted.contents().salt());
    }
}

#[test]
fn v1_vec_buffer_resize() {
    let mut rng = StdRng::from_entropy();
    let identities = TestIdentities::generate::<1, _>(&mut rng);
    let buffer = VecBuffer::new(TEST_BUFFER_SIZE);
    let mut adv_builder = AdvBuilder::<VecBuffer>::new(buffer);

    let num_sections = 6;
    for _ in 0..num_sections {
        let _ = add_mic_rand_length_salt_to_adv::<_, CryptoProviderImpl, 1, VecBuffer>(
            &mut rng,
            &identities.0[0],
            &mut adv_builder,
        )
        .unwrap();
    }

    let adv = adv_builder.into_advertisement();
    let mut buffer = VecBuffer::new(adv.as_slice().len());
    let arena = deserialization_arena!(&mut buffer);
    let cred_book = identities.build_cred_book::<CryptoProviderImpl>();

    let v1_contents = deser_v1::<_, _, CryptoProviderImpl>(arena, adv.as_slice(), &cred_book);
    assert_eq!(0, v1_contents.invalid_sections_count());
    assert_eq!(num_sections, v1_contents.sections().len());
}
