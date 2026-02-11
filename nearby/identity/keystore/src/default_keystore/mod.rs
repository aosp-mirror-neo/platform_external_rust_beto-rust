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

use crate::{KeyStore, KeyStoreError};
use crypto_provider::{
    aead::{Aead, AeadInit},
    aes::{Aes256Key, AesKey},
    CryptoProvider, CryptoRng,
};
use std::marker::PhantomData;
use storage::{Key, PreferenceKeyValueStorage as _};

const KEYSTORE_FILE_PATH: &str = "Google/Nearby/identity/keystore.json";

mod values {
    #[allow(dead_code)]
    pub trait Sealed {}
}

/// Marker trait that shows this preference is a part of Nearby identity keystore preferences.
#[allow(dead_code)]
pub(crate) trait KeystorePreference: values::Sealed + Sized + Key {
    fn default_value() -> <Self as Key>::Value;
}
/// Device name preference.
pub(crate) struct KeystoreSecretKey {}
impl values::Sealed for KeystoreSecretKey {}
impl KeystorePreference for KeystoreSecretKey {
    fn default_value() -> <Self as Key>::Value {
        String::new()
    }
}
impl Key for KeystoreSecretKey {
    const KEY: &'static str = "keystore_secret_key";
    type Value = String;
}

fn read_key_from_storage() -> Result<Option<Aes256Key>, KeyStoreError> {
    let local_app_data_dir =
        storage::get_local_app_data_dir().map_err(|_| KeyStoreError::Unexpected)?;
    let full_path = local_app_data_dir.join(KEYSTORE_FILE_PATH);
    let repository = storage::get_store(full_path);
    if let Some(secret_key) = repository.get::<KeystoreSecretKey>() {
        if secret_key.is_empty() {
            return Ok(None);
        }
        let key_bytes = hex::decode(secret_key)
            .map_err(|_| KeyStoreError::CryptoError("Failed to decode key".to_string()))?;
        let key_array: [u8; 32] = key_bytes
            .try_into()
            .map_err(|_| KeyStoreError::CryptoError("Invalid key length".to_string()))?;
        Ok(Some(Aes256Key::from(key_array)))
    } else {
        Ok(None)
    }
}

fn write_key_to_storage(key: &Aes256Key) -> Result<(), KeyStoreError> {
    let local_app_data_dir =
        storage::get_local_app_data_dir().map_err(|_| KeyStoreError::Unexpected)?;
    let full_path = local_app_data_dir.join(KEYSTORE_FILE_PATH);
    let repository = storage::get_store(full_path);
    let content = hex::encode(key.as_slice());
    repository
        .set::<KeystoreSecretKey>(content)
        .map_err(|e| KeyStoreError::CryptoError(format!("Failed to write key: {e}")))
}

fn clear_storage() -> Result<(), KeyStoreError> {
    let local_app_data_dir =
        storage::get_local_app_data_dir().map_err(|_| KeyStoreError::Unexpected)?;
    let full_path = local_app_data_dir.join(KEYSTORE_FILE_PATH);
    let repository = storage::get_store(full_path);
    repository
        .set::<KeystoreSecretKey>("".to_string())
        .map_err(|e| KeyStoreError::CryptoError(format!("Failed to clear storage: {e}")))
}

/// A default implementation of the `KeyStore` trait that manages a single secret key.
pub struct DefaultKeyStore<C: CryptoProvider> {
    _crypto_provider: PhantomData<C>,
}

impl<C: CryptoProvider> Default for DefaultKeyStore<C> {
    fn default() -> Self {
        Self { _crypto_provider: PhantomData }
    }
}

impl<C: CryptoProvider + Sync + 'static> KeyStore<C> for DefaultKeyStore<C> {
    fn ensure_key_exists() -> Result<(), KeyStoreError> {
        if read_key_from_storage()?.is_none() {
            let mut rng = C::CryptoRng::new();
            let key_bytes: [u8; 32] = rng.gen();
            let key = Aes256Key::from(key_bytes);
            write_key_to_storage(&key)?;
        }
        Ok(())
    }

    fn encrypt(plaintext: &[u8]) -> Result<Vec<u8>, KeyStoreError> {
        let secret_key = read_key_from_storage()?.ok_or(KeyStoreError::KeyNotFound)?;

        let aead = <C::Aes256Gcm as AeadInit<_>>::new(&secret_key);
        let mut nonce_bytes = [0u8; 12];
        C::CryptoRng::new().fill(&mut nonce_bytes);
        let nonce = &nonce_bytes;

        let ciphertext = aead
            .encrypt(plaintext, &[], nonce)
            .map_err(|_| KeyStoreError::CryptoError("Encryption failed".to_string()))?;

        let mut result = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    fn decrypt(ciphertext: &[u8]) -> Result<Vec<u8>, KeyStoreError> {
        const NONCE_SIZE: usize = 12;
        if ciphertext.len() < NONCE_SIZE {
            return Err(KeyStoreError::CryptoError(
                "Encrypted data is too short to contain a nonce.".to_string(),
            ));
        }

        let secret_key = read_key_from_storage()?.ok_or(KeyStoreError::KeyNotFound)?;

        let aead = <C::Aes256Gcm as AeadInit<_>>::new(&secret_key);

        let (nonce_slice, encrypted_data) = ciphertext.split_at(NONCE_SIZE);
        let nonce_array: [u8; NONCE_SIZE] = nonce_slice.try_into().map_err(|_| {
            KeyStoreError::CryptoError("Failed to extract nonce from ciphertext".to_string())
        })?;

        aead.decrypt(encrypted_data, &[], &nonce_array)
            .map_err(|_| KeyStoreError::CryptoError("Decryption failed".to_string()))
    }

    fn delete_key() -> Result<(), KeyStoreError> {
        if read_key_from_storage()?.is_some() {
            clear_storage()
        } else {
            Err(KeyStoreError::KeyNotFound)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto_provider_default::CryptoProviderImpl;
    use serial_test::serial;

    fn setup() -> Result<(), KeyStoreError> {
        // This is to ensure the key storage is clean for each test.
        if read_key_from_storage()?.is_some() {
            clear_storage()?;
        }
        Ok(())
    }

    #[test]
    #[serial]
    fn test_ensure_key_exists_creates_key() -> Result<(), KeyStoreError> {
        setup()?;
        DefaultKeyStore::<CryptoProviderImpl>::ensure_key_exists()?;
        let key = read_key_from_storage()?;
        assert!(key.is_some());
        Ok(())
    }

    #[test]
    #[serial]
    fn test_encrypt_decrypt_private_key() -> Result<(), KeyStoreError> {
        setup()?;
        DefaultKeyStore::<CryptoProviderImpl>::ensure_key_exists()?;
        let raw_private_key = [42u8; 32];

        let encrypted_key = DefaultKeyStore::<CryptoProviderImpl>::encrypt(&raw_private_key)?;
        let decrypted_key = DefaultKeyStore::<CryptoProviderImpl>::decrypt(&encrypted_key)?;

        assert_eq!(decrypted_key, raw_private_key);
        Ok(())
    }

    #[test]
    #[serial]
    fn test_key_is_persistent_in_keystore() -> Result<(), KeyStoreError> {
        setup()?;
        DefaultKeyStore::<CryptoProviderImpl>::ensure_key_exists()?;
        let raw_private_key1 = [42u8; 32];
        let raw_private_key2 = [24u8; 32];

        let encrypted_key1 = DefaultKeyStore::<CryptoProviderImpl>::encrypt(&raw_private_key1)?;
        let encrypted_key2 = DefaultKeyStore::<CryptoProviderImpl>::encrypt(&raw_private_key2)?;

        let decrypted_key1 = DefaultKeyStore::<CryptoProviderImpl>::decrypt(&encrypted_key1)?;
        let decrypted_key2 = DefaultKeyStore::<CryptoProviderImpl>::decrypt(&encrypted_key2)?;

        assert_eq!(decrypted_key1, raw_private_key1);
        assert_eq!(decrypted_key2, raw_private_key2);
        Ok(())
    }

    #[test]
    #[serial]
    fn test_decrypt_with_invalid_data_errors() -> Result<(), KeyStoreError> {
        setup()?;
        DefaultKeyStore::<CryptoProviderImpl>::ensure_key_exists()?;
        // Using a key that is too short to be valid
        let invalid_encrypted_key = vec![1, 2, 3];
        let result = DefaultKeyStore::<CryptoProviderImpl>::decrypt(&invalid_encrypted_key);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    #[serial]
    fn test_delete_key() -> Result<(), KeyStoreError> {
        setup()?;
        DefaultKeyStore::<CryptoProviderImpl>::ensure_key_exists()?;
        DefaultKeyStore::<CryptoProviderImpl>::delete_key()?;
        let key = read_key_from_storage()?;
        assert!(key.is_none());
        Ok(())
    }

    #[test]
    #[serial]
    fn test_delete_non_existent_key_errors() -> Result<(), KeyStoreError> {
        setup()?;
        assert!(DefaultKeyStore::<CryptoProviderImpl>::delete_key().is_err());
        Ok(())
    }

    #[test]
    fn test_keystore_secret_key_default_value() {
        assert_eq!(KeystoreSecretKey::default_value(), "");
    }
}
