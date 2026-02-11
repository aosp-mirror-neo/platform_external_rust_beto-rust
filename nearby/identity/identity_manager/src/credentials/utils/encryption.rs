// Copyright 2025 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
//
// you may not use this file except in compliance with the License.
//
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
//
// distributed under the License is distributed on an "AS IS" BASIS,
//
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//
// See the License for the specific language governing permissions and
//
// limitations under the License.
//
//

// Use traits from the crypto_provider library instead of concrete implementations.
use crypto_provider::{sha2::Sha256, CryptoProvider, CryptoRng};
use ldt_np_adv::V0_IDENTITY_TOKEN_LEN;
use np_hkdf::DerivedSectionKeys as _;
use up_adv::extended::V1_IDENTITY_TOKEN_LEN;

const KEY_SEED_BYTE_SIZE: usize = 32;

/// Creates a key seed of a given size using the provider's secure random number generator.
pub(super) fn create_key_seed<C: CryptoProvider>(size: usize) -> Vec<u8> {
    let mut key = vec![0u8; size];
    let mut rng = C::CryptoRng::new();
    rng.fill(&mut key);
    key
}

/// Generates a credential ID from a key seed using the provider's SHA-256 implementation.
pub fn generate_id_for_credential<C: CryptoProvider>(key_seed: &[u8]) -> Option<Vec<u8>> {
    if key_seed.len() != KEY_SEED_BYTE_SIZE {
        return None;
    }
    // Use the one-shot SHA256 function from the provider.
    let digest = C::Sha256::sha256(key_seed);
    Some(digest.to_vec())
}

/// Creates a tag for the V0 metadata encryption key using the provider's HKDF-SHA256.
pub(super) fn create_identity_token_hmac_key<C: CryptoProvider>(
    hkdf: &np_hkdf::NpKeySeedHkdf<C>,
    identity_token_v0: &[u8; V0_IDENTITY_TOKEN_LEN],
) -> Vec<u8> {
    hkdf.v0_identity_token_hmac_key().calculate_hmac::<C>(identity_token_v0).to_vec()
}

pub(super) fn create_identity_token_short_salt_adv_hmac_key<C: CryptoProvider>(
    hkdf: &np_hkdf::NpKeySeedHkdf<C>,
    identity_token_v1: &[u8; V1_IDENTITY_TOKEN_LEN],
) -> Vec<u8> {
    hkdf.v1_mic_short_salt_keys()
        .identity_token_hmac_key()
        .calculate_hmac::<C>(identity_token_v1)
        .to_vec()
}

pub(super) fn create_identity_token_extended_salt_adv_hmac_key<C: CryptoProvider>(
    hkdf: &np_hkdf::NpKeySeedHkdf<C>,
    identity_token_v1: &[u8; V1_IDENTITY_TOKEN_LEN],
) -> Vec<u8> {
    hkdf.v1_mic_extended_salt_keys()
        .identity_token_hmac_key()
        .calculate_hmac::<C>(identity_token_v1)
        .to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto_provider_default::CryptoProviderImpl;
    use hex_literal::hex;

    #[test]
    fn test_create_key_seed() {
        let key_seed1 = create_key_seed::<CryptoProviderImpl>(32);
        let key_seed2 = create_key_seed::<CryptoProviderImpl>(32);
        assert_eq!(key_seed1.len(), 32);
        assert_eq!(key_seed2.len(), 32);
        assert_ne!(key_seed1, key_seed2);
    }

    #[test]
    fn test_generate_id_for_credential() {
        let key_seed = hex!("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f");
        #[allow(clippy::unwrap_used)]
        let cred_id = generate_id_for_credential::<CryptoProviderImpl>(&key_seed).unwrap();
        assert_eq!(
            cred_id,
            hex!("630dcd2966c4336691125448bbb25b4ff412a49c732db2c8abc1b8581bd710dd")
        );
    }

    #[test]
    fn test_generate_id_for_credential_invalid_seed_len() {
        let key_seed = &[0; 31];
        assert!(generate_id_for_credential::<CryptoProviderImpl>(key_seed).is_none());
        let key_seed = &[0; 33];
        assert!(generate_id_for_credential::<CryptoProviderImpl>(key_seed).is_none());
    }
}
