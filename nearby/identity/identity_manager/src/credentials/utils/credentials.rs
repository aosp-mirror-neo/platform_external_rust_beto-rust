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

use super::encryption;
use crate::errors::CredentialsError;
use crate::types::AuthToken;
use chrono::{DateTime, Duration, Utc};
use credential_proto::proto::{
    broadcast_credential::BroadcastCredential, credential::DiscoveryCredential,
    metadata::DeviceIdentityMetaData,
};
use crypto_provider::{
    self,
    ed25519::{self, Ed25519Provider},
    sha2::Sha256,
    CryptoProvider,
};
use ldt_np_adv::{V0IdentityToken, V0_IDENTITY_TOKEN_LEN};
use protobuf::Message;
use std::collections::HashMap;
use up_adv::{
    credential::{metadata, v0::V0, v1::V1},
    extended::{V1IdentityToken, V1_IDENTITY_TOKEN_LEN},
};

// --- Constants, unchanged ---
const KEY_SEED_BYTE_SIZE: usize = 32;
const IDENTITY_TOKEN_V0_BYTE_SIZE: usize = 14;
const IDENTITY_TOKEN_V1_BYTE_SIZE: usize = 16;
const CREDENTIAL_EXPIRATION_DURATION_MS: i64 = 5 * 24 * 60 * 60 * 1000; // 5 days.
const CREDENTIAL_ACTIVE_TIME_OFFSET_MILLIS: i64 = 30 * 60 * 1000; // 30 minutes.

//TODO: replace these params with phenotype flags:
const CURRENT_CREDENTIAL_VERSION: i64 = 3;
const REGENERATE_CREDENTIALS_TO_LATEST_VERSION: bool = false;

/// Creates a pair of Local and Shared credentials.
/// This version is generic over a CryptoProvider implementation.
pub(crate) fn create_credential_pair<C: CryptoProvider>(
    start_time: DateTime<Utc>,
    metadata: &DeviceIdentityMetaData,
) -> Result<(BroadcastCredential, DiscoveryCredential), CredentialsError>
where
    C::Ed25519: Ed25519Provider,
{
    let key_seed = encryption::create_key_seed::<C>(KEY_SEED_BYTE_SIZE);
    let id_bytes = encryption::generate_id_for_credential::<C>(&key_seed)
        .ok_or(CredentialsError::InvalidCredentialMaterial)?;
    let id = extract_long_from_bytes(&id_bytes);
    let end_time = start_time + Duration::milliseconds(CREDENTIAL_EXPIRATION_DURATION_MS);

    let broadcast_credential = BroadcastCredential {
        id,
        start_time_millis: start_time.timestamp_millis(),
        end_time_millis: end_time.timestamp_millis(),
        key_seed,
        identity_token_v0: encryption::create_key_seed::<C>(IDENTITY_TOKEN_V0_BYTE_SIZE),
        identity_token_v1: encryption::create_key_seed::<C>(IDENTITY_TOKEN_V1_BYTE_SIZE),
        credential_version: CURRENT_CREDENTIAL_VERSION,
        ..Default::default()
    };

    // 8. Convert to DiscoveryCredential, passing the provider's public key
    let discovery_credential = convert::<C>(&broadcast_credential, metadata)?;

    Ok((broadcast_credential, discovery_credential))
}

/// Extracts a 64-bit integer from the beginning of a byte slice.
/// (No crypto dependencies, unchanged).
fn extract_long_from_bytes(bytes: &[u8]) -> i64 {
    let mut long_bytes = [0u8; 8];
    let len_to_copy = std::cmp::min(bytes.len(), 8);
    #[allow(clippy::indexing_slicing)]
    long_bytes[..len_to_copy].copy_from_slice(&bytes[..len_to_copy]);
    i64::from_be_bytes(long_bytes)
}

/// Helper function to convert a BroadcastCredential to a DiscoveryCredential.
fn convert<C: CryptoProvider>(
    local: &BroadcastCredential,
    metadata: &DeviceIdentityMetaData,
) -> Result<DiscoveryCredential, CredentialsError> {
    let key_seed: &[u8; KEY_SEED_BYTE_SIZE] = local
        .key_seed
        .as_slice()
        .try_into()
        .map_err(|_| CredentialsError::InvalidCredentialMaterial)?;
    let identity_token_v0: [u8; V0_IDENTITY_TOKEN_LEN] = local
        .identity_token_v0
        .clone()
        .try_into()
        .map_err(|_| CredentialsError::InvalidCredentialMaterial)?;
    let identity_token_v1: [u8; V1_IDENTITY_TOKEN_LEN] = local
        .identity_token_v1
        .clone()
        .try_into()
        .map_err(|_| CredentialsError::InvalidCredentialMaterial)?;
    let hkdf = np_hkdf::NpKeySeedHkdf::<C>::new(key_seed);
    let metadata_bytes =
        metadata.write_to_bytes().map_err(|_| CredentialsError::InvalidCredentialMaterial)?;
    let encrypted_metadata_bytes_v0 = metadata::encrypt_metadata::<C, V0>(
        &hkdf,
        V0IdentityToken::from(identity_token_v0),
        metadata_bytes.as_slice(),
    );
    let encrypted_metadata_bytes_v1 = metadata::encrypt_metadata::<C, V1>(
        &hkdf,
        V1IdentityToken::from(identity_token_v1),
        metadata_bytes.as_slice(),
    );
    let identity_token_hmac_key_v0 =
        encryption::create_identity_token_hmac_key::<C>(&hkdf, &identity_token_v0);
    let identity_token_short_salt_adv_hmac_key_v1 =
        encryption::create_identity_token_short_salt_adv_hmac_key::<C>(&hkdf, &identity_token_v1);
    let identity_token_extended_salt_adv_hmac_key_v1 =
        encryption::create_identity_token_extended_salt_adv_hmac_key::<C>(
            &hkdf,
            &identity_token_v1,
        );
    let visibility_scope = metadata.visibility_scope.clone();
    if visibility_scope.is_none() {
        log::warn!("Device metadata must contain a visibility scope for credential generation!");
        return Err(CredentialsError::InvalidCredentialMaterial);
    };

    Ok(DiscoveryCredential {
        id: local.id,
        key_seed: local.key_seed.clone(),
        encrypted_metadata_bytes_v0,
        encrypted_metadata_bytes_v1,
        start_time_millis: local.start_time_millis,
        end_time_millis: local.end_time_millis,
        identity_token_hmac_key_v0,
        identity_token_short_salt_adv_hmac_key_v1,
        identity_token_extended_salt_adv_hmac_key_v1,
        credential_version: CURRENT_CREDENTIAL_VERSION,
        visibility_scope,
        ..Default::default()
    })
}

pub(crate) fn are_broadcast_credentials_valid(creds: &[BroadcastCredential]) -> bool {
    for cred in creds {
        if !is_broadcast_credential_valid(cred) {
            return false;
        }
    }
    true
}

fn is_broadcast_credential_valid(credential: &BroadcastCredential) -> bool {
    // Check crypto field sizes.
    if credential.key_seed.len() != KEY_SEED_BYTE_SIZE
        || credential.identity_token_v0.len() != IDENTITY_TOKEN_V0_BYTE_SIZE
        || credential.identity_token_v1.len() != IDENTITY_TOKEN_V1_BYTE_SIZE
    {
        log::warn!("Credential not advertiseable due to incorrect key material size.");
        return false;
    }

    // Check credential version for rollback.
    if credential.credential_version > CURRENT_CREDENTIAL_VERSION {
        log::warn!(
            "Credential version has been rolled back! Force regenerate all credentials with default version."
        );
        return false;
    }

    // Check for dogfooding/upgrade condition.
    if REGENERATE_CREDENTIALS_TO_LATEST_VERSION
        && (credential.credential_version != CURRENT_CREDENTIAL_VERSION)
    {
        log::warn!(
            "Dogfood testing credential upgrade: this device is force upgrading credentials to the latest released version."
        );
        return false;
    }

    true
}

pub(crate) fn is_expired_credential(broadcast_credential: &BroadcastCredential) -> bool {
    broadcast_credential.end_time_millis <= Utc::now().timestamp_millis()
}

pub(crate) fn find_active_credential(
    creds: &[BroadcastCredential],
) -> Option<&BroadcastCredential> {
    creds.iter().find(|cred| is_active_credential(cred.start_time_millis, cred.end_time_millis))
}

pub(crate) fn find_mut_active_credential(
    creds: &mut [BroadcastCredential],
) -> Option<&mut BroadcastCredential> {
    creds.iter_mut().find(|cred| is_active_credential(cred.start_time_millis, cred.end_time_millis))
}

fn is_active_credential(start_time_millis: i64, end_time_millis: i64) -> bool {
    let now = Utc::now().timestamp_millis();
    ensure_valid_timestamp(start_time_millis, -CREDENTIAL_ACTIVE_TIME_OFFSET_MILLIS) <= now
        && now < ensure_valid_timestamp(end_time_millis, CREDENTIAL_ACTIVE_TIME_OFFSET_MILLIS)
}

/// Re-encrypts the metadata of a `DiscoveryCredential` using its corresponding `BroadcastCredential`
/// and new metadata.
pub(crate) fn replace_credential_encrypted_metadata<C: CryptoProvider>(
    broadcast_cred: &BroadcastCredential,
    shared_cred: &DiscoveryCredential,
    new_metadata: &DeviceIdentityMetaData,
) -> DiscoveryCredential {
    #[allow(clippy::expect_used)]
    let key_seed: &[u8; KEY_SEED_BYTE_SIZE] = broadcast_cred
        .key_seed
        .as_slice()
        .try_into()
        .expect("Key seed length should never be incorrect.");
    #[allow(clippy::expect_used)]
    let identity_token_v0: [u8; V0_IDENTITY_TOKEN_LEN] = broadcast_cred
        .identity_token_v0
        .clone()
        .try_into()
        .expect("V0 identity token length should never be incorrect.");
    #[allow(clippy::expect_used)]
    let identity_token_v1: [u8; V1_IDENTITY_TOKEN_LEN] = broadcast_cred
        .identity_token_v1
        .clone()
        .try_into()
        .expect("V0 identity token length should never be incorrect.");

    let hkdf = np_hkdf::NpKeySeedHkdf::<C>::new(key_seed);
    let metadata_bytes = new_metadata.write_to_bytes().unwrap_or_default();

    let encrypted_metadata_bytes_v0 = metadata::encrypt_metadata::<C, V0>(
        &hkdf,
        V0IdentityToken::from(identity_token_v0),
        metadata_bytes.as_slice(),
    );
    let encrypted_metadata_bytes_v1 = metadata::encrypt_metadata::<C, V1>(
        &hkdf,
        V1IdentityToken::from(identity_token_v1),
        metadata_bytes.as_slice(),
    );

    DiscoveryCredential {
        encrypted_metadata_bytes_v0,
        encrypted_metadata_bytes_v1,
        ..shared_cred.clone()
    }
}

fn ensure_valid_timestamp(time_millis: i64, offset: i64) -> i64 {
    time_millis.saturating_add(offset)
}

pub(crate) fn extract_auth_token<C: CryptoProvider>(key_seed: &[u8]) -> AuthToken {
    let hash = C::Sha256::sha256(key_seed);
    // The `try_into` is safe because the hash is guaranteed to be 32 bytes long,
    // so the first 8 bytes are always available.
    #[allow(clippy::expect_used)]
    let auth_token_bytes: [u8; 8] = hash[..8]
        .try_into()
        .expect("sha256 hash is 32 bytes, so first 8 bytes are always available");
    AuthToken::new(u64::from_be_bytes(auth_token_bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use credential_proto::proto::{
        credential::{
            DiscoveryCredential, VisibilityLevel, VisibilityScope as ProtoVisibilityScope,
        },
        metadata::DeviceIdentityMetaData,
    };
    use crypto_provider_default::CryptoProviderImpl;

    #[allow(clippy::unwrap_used)]
    #[test]
    fn test_create_credential_pair() {
        let start_time = Utc::now();
        let metadata = DeviceIdentityMetaData {
            visibility_scope: protobuf::MessageField::some(ProtoVisibilityScope::default()),
            ..Default::default()
        };
        let result = create_credential_pair::<CryptoProviderImpl>(start_time, &metadata);
        assert!(result.is_ok());
        let (local, shared) = result.unwrap();
        assert_eq!(local.id, shared.id);
        assert_eq!(local.start_time_millis, shared.start_time_millis);
        assert_eq!(local.end_time_millis, shared.end_time_millis);
        assert_eq!(local.key_seed, shared.key_seed);
    }

    #[allow(clippy::unwrap_used)]
    #[test]
    fn test_convert() {
        let start_time = Utc::now();
        let metadata = DeviceIdentityMetaData {
            visibility_scope: protobuf::MessageField::some(ProtoVisibilityScope::default()),
            ..Default::default()
        };
        let result = create_credential_pair::<CryptoProviderImpl>(start_time, &metadata);
        assert!(result.is_ok());
        let (local, _) = result.unwrap();
        let shared = convert::<CryptoProviderImpl>(&local, &metadata);
        assert!(shared.is_ok());
        let shared = shared.unwrap();
        assert_eq!(local.id, shared.id);
        assert_eq!(local.start_time_millis, shared.start_time_millis);
        assert_eq!(local.end_time_millis, shared.end_time_millis);
        assert_eq!(local.key_seed, shared.key_seed);
    }

    #[allow(clippy::unwrap_used)]
    #[test]
    fn test_replace_credential_encrypted_metadata() {
        // 1. Create an initial credential pair.
        let start_time = Utc::now();
        let initial_metadata = DeviceIdentityMetaData {
            visibility_scope: protobuf::MessageField::some(ProtoVisibilityScope {
                level: protobuf::EnumOrUnknown::new(VisibilityLevel::VISIBILITY_LEVEL_SELF_GROUP),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (broadcast_cred, shared_cred) =
            create_credential_pair::<CryptoProviderImpl>(start_time, &initial_metadata).unwrap();

        // 2. Create new metadata.
        let new_metadata = DeviceIdentityMetaData {
            visibility_scope: protobuf::MessageField::some(ProtoVisibilityScope {
                level: protobuf::EnumOrUnknown::new(
                    VisibilityLevel::VISIBILITY_LEVEL_CONTACTS_GROUP,
                ),
                ..Default::default()
            }),
            ..Default::default()
        };

        // 3. Call the function to re-encrypt metadata.
        let updated_shared_cred = replace_credential_encrypted_metadata::<CryptoProviderImpl>(
            &broadcast_cred,
            &shared_cred,
            &new_metadata,
        );

        // 4. Assert that the encrypted metadata has changed.
        assert_ne!(
            updated_shared_cred.encrypted_metadata_bytes_v0,
            shared_cred.encrypted_metadata_bytes_v0
        );
        assert_ne!(
            updated_shared_cred.encrypted_metadata_bytes_v1,
            shared_cred.encrypted_metadata_bytes_v1
        );

        // 5. Assert that other fields remain the same.
        assert_eq!(updated_shared_cred.id, shared_cred.id);
        assert_eq!(updated_shared_cred.key_seed, shared_cred.key_seed);
        assert_eq!(
            updated_shared_cred.connection_signature_verification_key,
            shared_cred.connection_signature_verification_key
        );
        assert_eq!(updated_shared_cred.start_time_millis, shared_cred.start_time_millis);
        assert_eq!(updated_shared_cred.end_time_millis, shared_cred.end_time_millis);
        assert_eq!(
            updated_shared_cred.identity_token_hmac_key_v0,
            shared_cred.identity_token_hmac_key_v0
        );
        assert_eq!(
            updated_shared_cred.identity_token_short_salt_adv_hmac_key_v1,
            shared_cred.identity_token_short_salt_adv_hmac_key_v1
        );
        assert_eq!(
            updated_shared_cred.identity_token_extended_salt_adv_hmac_key_v1,
            shared_cred.identity_token_extended_salt_adv_hmac_key_v1
        );
        assert_eq!(updated_shared_cred.credential_version, shared_cred.credential_version);
        assert_eq!(updated_shared_cred.visibility_scope, shared_cred.visibility_scope);
    }

    #[test]
    fn test_extract_auth_token() {
        let key_seed = b"test key seed";
        let auth_token = extract_auth_token::<CryptoProviderImpl>(key_seed);

        // Corresponds to the first 8 bytes of SHA256("test key seed") in big-endian.
        // SHA256: f7556c8109956ae46fda1f295821ff414d755d2ed6bfc45455ecbdb2b813916f
        let expected_bytes = [0xf7, 0x55, 0x6c, 0x81, 0x09, 0x95, 0x6a, 0xe4];
        let expected_auth_token = AuthToken::new(u64::from_be_bytes(expected_bytes));

        assert_eq!(auth_token, expected_auth_token);
    }
}
