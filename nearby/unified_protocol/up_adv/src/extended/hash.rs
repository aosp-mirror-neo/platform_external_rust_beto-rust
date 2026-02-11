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

//! Hashing functions for extended advertisements.

use crypto_provider::{hkdf::Hkdf, CryptoProvider};
use sink::Sink;
use tinyvec::ArrayVec;

/// 3-byte Fixed Identity Hash
pub type FixedIdentityHash = [u8; 3];

const MAX_SERVICE_ID_LEN: usize = 256;
const IKM_CAPACITY: usize = 32 + MAX_SERVICE_ID_LEN;
const VLQ_U64_MAX_LEN: usize = 10;

/// Derive a fixed identity hash from a discovery credential key seed and a service ID.
pub fn derive_fixed_identity_hash<C: CryptoProvider>(
    key_seed: &[u8; 32],
    service_id: &[u8],
    time_slot: u64,
) -> Option<FixedIdentityHash> {
    if service_id.len() > MAX_SERVICE_ID_LEN {
        return None; // Service ID too long
    }

    let mut ikm = ArrayVec::<[u8; IKM_CAPACITY]>::new();
    ikm.try_extend_from_slice(key_seed)?;
    ikm.try_extend_from_slice(service_id)?;

    let salt = encode_vlq(time_slot);

    let hkdf = C::HkdfSha256::new(Some(&salt), &ikm);
    let mut out = [0u8; 3];
    hkdf.expand(b"DeriveFixedIdentityHash", &mut out).ok()?;
    Some(out)
}

fn encode_vlq(mut value: u64) -> ArrayVec<[u8; VLQ_U64_MAX_LEN]> {
    let mut result = ArrayVec::<[u8; VLQ_U64_MAX_LEN]>::new();
    if value == 0 {
        result.push(0);
        return result;
    }

    let mut temp = ArrayVec::<[u8; VLQ_U64_MAX_LEN]>::new();
    while value > 0 {
        temp.push((value & 0x7F) as u8);
        value >>= 7;
    }

    // Reverse to get Big Endian order
    for &byte in temp.iter().rev() {
        result.push(byte);
    }

    // Set continuation bits
    let len = result.len();
    if len > 0 {
        for byte in result.iter_mut().take(len - 1) {
            *byte |= 0x80;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto_provider_default::CryptoProviderImpl;
    use tinyvec::array_vec;

    #[test]
    fn test_encode_vlq() {
        assert_eq!(encode_vlq(0x00), array_vec!([u8; 10] => 0x00));
        assert_eq!(encode_vlq(0x7F), array_vec!([u8; 10] => 0x7F));
        assert_eq!(encode_vlq(0x80), array_vec!([u8; 10] => 0x81, 0x00));
        assert_eq!(encode_vlq(0x2000), array_vec!([u8; 10] => 0xC0, 0x00));
        assert_eq!(encode_vlq(0xFFFF), array_vec!([u8; 10] => 0x83, 0xFF, 0x7F));
        assert_eq!(
            encode_vlq(u64::MAX),
            array_vec!([u8; 10] => 0x81, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F)
        );
    }

    #[test]
    fn test_derive_fixed_identity_hash() {
        let key_seed = [1u8; 32];
        let service_id = [2u8; 3];
        let time_slot = 12345;

        let hash =
            derive_fixed_identity_hash::<CryptoProviderImpl>(&key_seed, &service_id, time_slot)
                .expect("Hash derivation failed in test");
        assert_eq!(hash.len(), 3);
        // We don't have a test vector, but we ensure it runs and returns 3 bytes.
    }

    #[test]
    fn test_derive_fixed_identity_hash_max_sid() {
        let key_seed = [1u8; 32];
        let service_id = [2u8; MAX_SERVICE_ID_LEN];
        let time_slot = 12345;

        let hash =
            derive_fixed_identity_hash::<CryptoProviderImpl>(&key_seed, &service_id, time_slot)
                .expect("Hash derivation failed in test");
        assert_eq!(hash.len(), 3);
    }

    #[test]
    fn test_derive_fixed_identity_hash_sid_too_long() {
        let key_seed = [1u8; 32];
        let service_id = [2u8; MAX_SERVICE_ID_LEN + 1];
        let time_slot = 12345;

        let hash =
            derive_fixed_identity_hash::<CryptoProviderImpl>(&key_seed, &service_id, time_slot);
        assert!(hash.is_none());
    }
}
