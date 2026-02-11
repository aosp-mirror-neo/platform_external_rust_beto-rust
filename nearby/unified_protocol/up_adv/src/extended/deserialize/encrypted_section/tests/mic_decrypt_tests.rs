// Copyright 2022 Google LLC
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

#![allow(clippy::unwrap_used)]

extern crate std;

use super::super::*;

use crate::buffer::StackBuffer;

use crate::{
    deserialization_arena,
    extended::{
        data_elements::TxPowerDataElement,
        de_type::{DeType, HasDEType},
        deserialize::{
            section::intermediate::{
                parse_sections, tests::IntermediateSectionExt, CiphertextSection,
            },
            DataElement, DataElementParseError, Section,
        },
        salt::{MultiSalt, ShortV1Salt, Unsalted, SHORT_SALT_LEN},
        serialize::{
            AdvBuilder, ByteSliceSinkWriter, MicEncryptedSectionEncoder, WriteDataElement,
        },
    },
    shared_data::TxPower,
    UpVersionHeader,
};
use crypto_provider_default::CryptoProviderImpl;
use np_hkdf::DerivedSectionKeys;

const TEST_BUFFER_SIZE: usize = 250;

#[test]
fn deserialize_mic_encrypted_correct_keys_derived_salt() {
    // For V1_ENCODING_ENCRYPTED (formerly Extended Salt), we use derived salt.
    // We pick a root salt and derive the section salt.
    let root_salt = [0xAA; 16];
    let derived_salt =
        crate::extended::salt::derive_section_salt::<CryptoProviderImpl>(&root_salt, 0).unwrap();

    // Use MultiSalt::Derived to trigger 0x01 encoding without inline salt
    deserialize_mic_encrypted_correct_keys(MultiSalt::Derived(derived_salt), Some(root_salt))
}

#[test]
fn deserialize_mic_encrypted_correct_keys_short_salt() {
    deserialize_mic_encrypted_correct_keys(ShortV1Salt::from([3; SHORT_SALT_LEN]).into(), None)
}

fn deserialize_mic_encrypted_correct_keys(salt: MultiSalt, root_salt: Option<[u8; 16]>) {
    let identity_token = V1IdentityToken::from([1; 16]);
    let key_seed = [2; 32];
    let broadcast_cred = V1BroadcastCredential::new(key_seed, identity_token);

    let buffer = StackBuffer::<TEST_BUFFER_SIZE>::new();
    let mut adv_builder = AdvBuilder::<StackBuffer<TEST_BUFFER_SIZE>>::new(buffer);

    let mut encoder =
        MicEncryptedSectionEncoder::<_>::new::<CryptoProviderImpl>(salt, &broadcast_cred);
    if root_salt.is_some() {
        // If we are using Derived salt (0x01), we expect the header to have Root Salt (0x10).
        // 0x20 (Version) | 0x10 (Root Salt) = 0x30.
        encoder.set_adv_header_byte(0x30);
    }

    let mut section_builder = adv_builder.section_builder(encoder).unwrap();

    let txpower_de = TxPowerDataElement::from(TxPower::try_from(5).unwrap());
    section_builder.add_de(&txpower_de).unwrap();
    section_builder.add_to_advertisement::<CryptoProviderImpl>();
    let adv = adv_builder.into_advertisement();

    let mut adv_bytes = adv.as_slice().to_vec();
    if let Some(rs) = root_salt {
        // V1 Header with Root Salt (Bit 4 set -> 0x10)
        // 0x20 (Version) | 0x10 (Root Salt) = 0x30.
        // Insert root salt after header byte.
        // AdvBuilder puts sections immediately after header byte.
        let mut new_bytes = Vec::new();
        new_bytes.push(0x30);
        new_bytes.extend_from_slice(&rs);
        new_bytes.extend_from_slice(&adv_bytes[1..]); // Skip old header byte
        adv_bytes = new_bytes;
    }

    let (remaining, header) = UpVersionHeader::parse(&adv_bytes).unwrap();

    let adv_header = if let UpVersionHeader::V1(h) = header {
        h
    } else {
        panic!("incorrect header");
    };

    let sections = parse_sections::<CryptoProviderImpl>(adv_header, remaining).unwrap();
    assert_eq!(1, sections.len());

    let section = sections.into_iter().next().unwrap();
    let enc_section = section.as_ciphertext().unwrap();

    let CiphertextSection::MicEncrypted(contents) = &enc_section;

    // deserializing to Section works
    let discovery_credential = V1BroadcastCredential::new(key_seed, identity_token)
        .derive_discovery_credential::<CryptoProviderImpl>();

    let mut buffer = StackBuffer::<TEST_BUFFER_SIZE>::new();
    let arena = deserialization_arena!(&mut buffer);
    let mut allocator = arena.into_allocator();
    let section = contents
        .try_resolve_identity_and_deserialize::<CryptoProviderImpl>(
            &mut allocator,
            &discovery_credential,
        )
        .unwrap();

    assert_eq!(
        DecryptedSection::new(
            VerificationMode::Mic,
            salt,
            identity_token,
            // Length one, unextended De header.
            &[0b00010000u8 + (TxPowerDataElement::DE_TYPE.as_u32() as u8), 5u8],
        ),
        section
    );
    let data_elements = section.collect_data_elements().unwrap();
    let maybe_extended_salt = match salt {
        MultiSalt::Short(_) => None,
        MultiSalt::Derived(salt) => Some(salt),
    };
    assert_eq!(data_elements, &[DataElement::new(0x05_u8.into(), &[5], maybe_extended_salt)]);

    // Check salt matches expected
    assert_eq!(contents.contents.salt(), salt);

    // plaintext is correct
    {
        let identity_resolution_contents =
            contents.contents.compute_identity_resolution_contents::<CryptoProviderImpl>();
        let identity_match = identity_resolution_contents
            .try_match::<CryptoProviderImpl>(&match salt {
                MultiSalt::Short(_) => discovery_credential
                    .mic_short_salt_identity_resolution_material::<CryptoProviderImpl>()
                    .into_raw_resolution_material(),
                MultiSalt::Derived(_) => discovery_credential
                    .mic_extended_salt_identity_resolution_material::<CryptoProviderImpl>()
                    .into_raw_resolution_material(),
            })
            .unwrap();
        let mut buffer = StackBuffer::<TEST_BUFFER_SIZE>::new();
        let arena = deserialization_arena!(&mut buffer);
        let mut allocator = arena.into_allocator();
        let decrypted = contents
            .contents
            .decrypt_ciphertext::<CryptoProviderImpl>(&mut allocator, identity_match)
            .unwrap();

        let mut expected = Vec::new();
        // battery de
        let _ = txpower_de.write_de(Unsalted, &mut expected);

        assert_eq!(identity_token, decrypted.identity_token);
        assert_eq!(&expected, decrypted.plaintext_contents);
    }
}

#[test]
fn deserialize_mic_encrypted_short_salt_incorrect_aes_key_error() {
    // bad aes key -> bad metadata key plaintext
    do_bad_deserialize_params::<CryptoProviderImpl>(
        ShortV1Salt::from([3; SHORT_SALT_LEN]).into(),
        IdentityResolutionOrDeserializationError::IdentityMatchingError,
        |cm| {
            cm.mic_short_salt_identity_resolution_material
                .as_mut_raw_resolution_material()
                .aes_key = [0xFF; 16].into();
        },
    );
}

#[test]
fn deserialize_mic_encrypted_short_salt_incorrect_identity_token_hmac_key_error() {
    // bad metadata key hmac key -> bad calculated metadata key mac
    do_bad_deserialize_params::<CryptoProviderImpl>(
        ShortV1Salt::from([3; SHORT_SALT_LEN]).into(),
        IdentityResolutionOrDeserializationError::IdentityMatchingError,
        |cm| {
            cm.mic_short_salt_identity_resolution_material
                .as_mut_raw_resolution_material()
                .identity_token_hmac_key = [0xFF; 32];
        },
    );
}

#[test]
fn deserialize_mic_encrypted_short_salt_incorrect_mic_hmac_key_error() {
    // bad mic hmac key -> bad calculated mic
    do_bad_deserialize_params::<CryptoProviderImpl>(
        ShortV1Salt::from([3; SHORT_SALT_LEN]).into(),
        MicVerificationError::MicMismatch.into(),
        |cm| {
            cm.mic_short_salt_verification_material.mic_hmac_key = [0xFF; 32];
        },
    );
}

#[test]
fn deserialize_mic_encrypted_short_salt_incorrect_expected_identity_token_hmac_error() {
    // bad expected metadata key mac
    do_bad_deserialize_params::<CryptoProviderImpl>(
        ShortV1Salt::from([3; SHORT_SALT_LEN]).into(),
        IdentityResolutionOrDeserializationError::IdentityMatchingError,
        |cm| {
            cm.mic_short_salt_identity_resolution_material
                .as_mut_raw_resolution_material()
                .expected_identity_token_hmac = [0xFF; 32];
        },
    );
}

#[test]
fn deserialize_mic_encrypted_short_salt_incorrect_aes_key_error_dupe() {
    // bad aes key -> bad metadata key plaintext
    do_bad_deserialize_params::<CryptoProviderImpl>(
        ShortV1Salt::from([3; SHORT_SALT_LEN]).into(),
        IdentityResolutionOrDeserializationError::IdentityMatchingError,
        |cm| {
            cm.mic_short_salt_identity_resolution_material
                .as_mut_raw_resolution_material()
                .aes_key = [0xFF; 16].into();
        },
    );
}

#[test]
fn deserialize_mic_encrypted_short_salt_incorrect_identity_token_hmac_key_error_dupe() {
    // bad metadata key hmac key -> bad calculated metadata key mac
    do_bad_deserialize_params::<CryptoProviderImpl>(
        ShortV1Salt::from([3; SHORT_SALT_LEN]).into(),
        IdentityResolutionOrDeserializationError::IdentityMatchingError,
        |cm| {
            cm.mic_short_salt_identity_resolution_material
                .as_mut_raw_resolution_material()
                .identity_token_hmac_key = [0xFF; 32];
        },
    );
}

#[test]
fn deserialize_mic_encrypted_short_salt_incorrect_mic_hmac_key_error_dupe() {
    // bad mic hmac key -> bad calculated mic
    do_bad_deserialize_params::<CryptoProviderImpl>(
        ShortV1Salt::from([3; SHORT_SALT_LEN]).into(),
        MicVerificationError::MicMismatch.into(),
        |cm| {
            cm.mic_short_salt_verification_material.mic_hmac_key = [0xFF; 32];
        },
    );
}

#[test]
fn deserialize_mic_encrypted_short_salt_incorrect_expected_identity_token_hmac_error_dupe() {
    // bad expected metadata key mac
    do_bad_deserialize_params::<CryptoProviderImpl>(
        ShortV1Salt::from([3; SHORT_SALT_LEN]).into(),
        IdentityResolutionOrDeserializationError::IdentityMatchingError,
        |cm| {
            cm.mic_short_salt_identity_resolution_material
                .as_mut_raw_resolution_material()
                .expected_identity_token_hmac = [0xFF; 32];
        },
    );
}

#[test]
fn deserialize_mic_encrypted_short_salt_incorrect_salt_error_dupe() {
    // bad salt -> bad iv -> bad metadata key plaintext
    do_bad_deserialize_tampered(
        ShortV1Salt::from([3; SHORT_SALT_LEN]).into(),
        DeserializeError::IdentityResolutionOrDeserializationError(
            IdentityResolutionOrDeserializationError::IdentityMatchingError,
        ),
        None,
        // replace the extended salt bytes
        |adv| adv[2..4].fill(0xFF),
    );
}

#[test]
fn deserialize_mic_encrypted_short_salt_de_that_wont_parse_dupe() {
    // add an extra byte to the section, leading it to try to parse a DE that doesn't exist
    do_bad_deserialize_tampered(
        ShortV1Salt::from([3; SHORT_SALT_LEN]).into(),
        DeserializeError::DataElementParseError(DataElementParseError::UnexpectedDataAfterEnd),
        Some(0xFF),
        |_| {},
    );
}

#[test]
fn deserialize_mic_encrypted_short_salt_tampered_mic_error_dupe() {
    // flip a bit in the first MIC byte
    do_bad_deserialize_tampered(
        ShortV1Salt::from([3; SHORT_SALT_LEN]).into(),
        DeserializeError::IdentityResolutionOrDeserializationError(
            MicVerificationError::MicMismatch.into(),
        ),
        None,
        |adv| {
            let mic_start = adv.len() - 16;
            adv[mic_start] ^= 0x01
        },
    );
}

#[test]
fn deserialize_mic_encrypted_short_salt_tampered_payload_error_dupe() {
    // flip the last payload bit
    do_bad_deserialize_tampered(
        ShortV1Salt::from([3; SHORT_SALT_LEN]).into(),
        DeserializeError::IdentityResolutionOrDeserializationError(
            MicVerificationError::MicMismatch.into(),
        ),
        None,
        |adv| {
            let before_mic = adv.len() - 17;
            adv[before_mic] ^= 0x01
        },
    );
}
#[test]
fn deserialize_mic_encrypted_short_salt_incorrect_salt_error() {
    // bad salt -> bad iv -> bad metadata key plaintext
    do_bad_deserialize_tampered(
        ShortV1Salt::from([3; SHORT_SALT_LEN]).into(),
        DeserializeError::IdentityResolutionOrDeserializationError(
            IdentityResolutionOrDeserializationError::IdentityMatchingError,
        ),
        None,
        // replace the extended salt bytes
        |adv| adv[2..4].fill(0xFF),
    );
}

#[test]
fn deserialize_mic_encrypted_short_salt_de_that_wont_parse() {
    // add an extra byte to the section, leading it to try to parse a DE that doesn't exist
    do_bad_deserialize_tampered(
        ShortV1Salt::from([3; SHORT_SALT_LEN]).into(),
        DeserializeError::DataElementParseError(DataElementParseError::UnexpectedDataAfterEnd),
        Some(0xFF),
        |_| {},
    );
}

#[test]
fn deserialize_mic_encrypted_short_salt_tampered_mic_error() {
    // flip a bit in the first MIC byte
    do_bad_deserialize_tampered(
        ShortV1Salt::from([3; SHORT_SALT_LEN]).into(),
        DeserializeError::IdentityResolutionOrDeserializationError(
            MicVerificationError::MicMismatch.into(),
        ),
        None,
        |adv| {
            let mic_start = adv.len() - 16;
            adv[mic_start] ^= 0x01
        },
    );
}

#[test]
fn deserialize_mic_encrypted_short_salt_tampered_payload_error() {
    // flip the last payload bit
    do_bad_deserialize_tampered(
        ShortV1Salt::from([3; SHORT_SALT_LEN]).into(),
        DeserializeError::IdentityResolutionOrDeserializationError(
            MicVerificationError::MicMismatch.into(),
        ),
        None,
        |adv| {
            let before_mic = adv.len() - 17;
            adv[before_mic] ^= 0x01
        },
    );
}

#[test]
fn arena_out_of_space_on_mic_verify() {
    let identity_token = V1IdentityToken::from([1; 16]);
    let key_seed = [2; 32];

    // Use derived salt
    let root_salt = [0xAA; 16];
    let section_salt =
        crate::extended::salt::derive_section_salt::<CryptoProviderImpl>(&root_salt, 0).unwrap();

    let broadcast_cm = V1BroadcastCredential::new(key_seed, identity_token);

    let buffer = StackBuffer::<TEST_BUFFER_SIZE>::new();
    let mut adv_builder = AdvBuilder::<StackBuffer<TEST_BUFFER_SIZE>>::new(buffer);

    let mut encoder = MicEncryptedSectionEncoder::<_>::new::<CryptoProviderImpl>(
        MultiSalt::Derived(section_salt),
        &broadcast_cm,
    );
    encoder.set_adv_header_byte(0x30);

    let mut section_builder = adv_builder.section_builder(encoder).unwrap();

    let txpower_de = TxPowerDataElement::from(TxPower::try_from(5).unwrap());
    section_builder.add_de(&txpower_de).unwrap();
    section_builder.add_to_advertisement::<CryptoProviderImpl>();
    let adv = adv_builder.into_advertisement();

    // Patch header
    let mut adv_bytes = adv.as_slice().to_vec();
    // 0x30 = V1 (0x20) | Root Salt (0x10)
    adv_bytes[0] = 0x30;
    let mut new_bytes = Vec::new();
    new_bytes.push(0x30);
    new_bytes.extend_from_slice(&root_salt);
    new_bytes.extend_from_slice(&adv_bytes[1..]);
    let adv_bytes = new_bytes;

    let (remaining, header) = UpVersionHeader::parse(&adv_bytes).unwrap();

    let adv_header = if let UpVersionHeader::V1(h) = header {
        h
    } else {
        panic!("incorrect header");
    };

    let sections = parse_sections::<CryptoProviderImpl>(adv_header, remaining).unwrap();
    assert_eq!(1, sections.len());

    let section = sections.into_iter().next().unwrap();
    let enc_section = section.as_ciphertext().unwrap();

    let CiphertextSection::MicEncrypted(contents) = &enc_section;

    // deserializing to Section works
    let discovery_credential = V1BroadcastCredential::new(key_seed, identity_token)
        .derive_discovery_credential::<CryptoProviderImpl>();

    let mut buffer = StackBuffer::<TEST_BUFFER_SIZE>::new();
    let arena = deserialization_arena!(&mut buffer);
    let mut allocator = arena.into_allocator();
    let _ = allocator.allocate(250).unwrap();
    let res = contents.try_resolve_identity_and_deserialize::<CryptoProviderImpl>(
        &mut allocator,
        &discovery_credential,
    );
    assert_eq!(
        Err(IdentityResolutionOrDeserializationError::DeserializationError(
            DeserializationError::ArenaOutOfSpace
        )),
        res
    );
}

/// Attempt a decryption that will fail when using the provided parameters for decryption only.
/// `None` means use the correct value for that parameter.
fn do_bad_deserialize_params<C: CryptoProvider>(
    salt: MultiSalt,
    error: IdentityResolutionOrDeserializationError<MicVerificationError>,
    mut mangle_crypto_material: impl FnMut(&mut PrecalculatedV1DiscoveryCryptoMaterial),
) {
    let identity_token = V1IdentityToken([1; 16]);
    let key_seed = [2; 32];
    let key_seed_hkdf = np_hkdf::NpKeySeedHkdf::<C>::new(&key_seed);

    let buffer = StackBuffer::<TEST_BUFFER_SIZE>::new();
    let mut adv_builder = AdvBuilder::<StackBuffer<TEST_BUFFER_SIZE>>::new(buffer);

    let broadcast_cm = V1BroadcastCredential::new(key_seed, identity_token);

    let mut section_builder = adv_builder
        .section_builder(MicEncryptedSectionEncoder::<_>::new::<CryptoProviderImpl>(
            salt,
            &broadcast_cm,
        ))
        .unwrap();

    section_builder.add_de(&TxPowerDataElement::from(TxPower::try_from(7).unwrap())).unwrap();

    section_builder.add_to_advertisement::<CryptoProviderImpl>();

    let adv = adv_builder.into_advertisement();

    let (remaining, header) = UpVersionHeader::parse(adv.as_slice()).unwrap();

    let v1_header = if let UpVersionHeader::V1(h) = header {
        h
    } else {
        panic!("incorrect header");
    };

    let sections = parse_sections::<CryptoProviderImpl>(v1_header, remaining).unwrap();
    assert_eq!(1, sections.len());

    let section = sections.into_iter().next().unwrap();
    let enc_section = section.as_ciphertext().unwrap();
    let CiphertextSection::MicEncrypted(contents) = &enc_section;

    // start with correct crypto material
    let mut crypto_material = V1DiscoveryCredential::new(
        key_seed,
        key_seed_hkdf
            .v1_mic_short_salt_keys()
            .identity_token_hmac_key()
            .calculate_hmac::<C>(&identity_token.0),
        key_seed_hkdf
            .v1_mic_extended_salt_keys()
            .identity_token_hmac_key()
            .calculate_hmac::<C>(&identity_token.0),
    )
    .to_precalculated::<C>();

    // then break it per the test case
    mangle_crypto_material(&mut crypto_material);

    let mut buffer = StackBuffer::<TEST_BUFFER_SIZE>::new();
    assert_eq!(
        error,
        contents
            .try_resolve_identity_and_deserialize::<C>(
                &mut deserialization_arena!(&mut buffer).into_allocator(),
                &crypto_material
            )
            .unwrap_err()
    );
}

#[derive(Debug, PartialEq)]
enum DeserializeError {
    IdentityResolutionOrDeserializationError(
        IdentityResolutionOrDeserializationError<MicVerificationError>,
    ),
    DataElementParseError(DataElementParseError),
}

fn do_bad_deserialize_tampered(
    salt: MultiSalt,
    expected_error: DeserializeError,
    mangle_section_extra_byte: Option<u8>,
    mangle_adv: impl Fn(&mut Vec<u8>),
) {
    let metadata_key = V1IdentityToken([1; 16]);
    let key_seed = [2; 32];

    let buffer = StackBuffer::<TEST_BUFFER_SIZE>::new();
    let mut adv_builder = AdvBuilder::<StackBuffer<TEST_BUFFER_SIZE>>::new(buffer);

    let broadcast_cm = V1BroadcastCredential::new(key_seed, metadata_key);

    let mut section_builder = adv_builder
        .section_builder(MicEncryptedSectionEncoder::<_>::new::<CryptoProviderImpl>(
            salt,
            &broadcast_cm,
        ))
        .unwrap();

    section_builder.add_de(&TxPowerDataElement::from(TxPower::try_from(7).unwrap())).unwrap();

    if let Some(mangle_section_extra_byte) = mangle_section_extra_byte {
        section_builder
            .section
            .write_chunk(
                DeType::try_from(0xFFFFFFFEu32).unwrap().into(),
                ByteSliceSinkWriter { slice: &[mangle_section_extra_byte] },
            )
            .unwrap();
    }

    section_builder.add_to_advertisement::<CryptoProviderImpl>();

    let adv = adv_builder.into_advertisement();
    let mut adv_mut = adv.as_slice().to_vec();
    mangle_adv(&mut adv_mut);

    let (remaining, header) = UpVersionHeader::parse(&adv_mut).unwrap();

    let v1_header = if let UpVersionHeader::V1(h) = header {
        h
    } else {
        panic!("incorrect header");
    };

    let sections = parse_sections::<CryptoProviderImpl>(v1_header, remaining).unwrap();
    assert_eq!(1, sections.len());

    let section = sections.into_iter().next().unwrap();
    let enc_section = section.as_ciphertext().unwrap();
    let CiphertextSection::MicEncrypted(contents) = &enc_section;

    let discovery_credential = V1BroadcastCredential::new(key_seed, metadata_key)
        .derive_discovery_credential::<CryptoProviderImpl>();

    let mut buffer = StackBuffer::<TEST_BUFFER_SIZE>::new();
    match contents.try_resolve_identity_and_deserialize::<CryptoProviderImpl>(
        &mut deserialization_arena!(&mut buffer).into_allocator(),
        &discovery_credential,
    ) {
        Ok(section) => {
            assert_eq!(
                expected_error,
                DeserializeError::DataElementParseError(
                    section.collect_data_elements().unwrap_err()
                )
            );
        }
        Err(e) => assert_eq!(
            expected_error,
            DeserializeError::IdentityResolutionOrDeserializationError(e),
        ),
    };
}
