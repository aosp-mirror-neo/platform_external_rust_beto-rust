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
//! A crate for managing cryptographic keys.

use crypto_provider::CryptoProvider;

/// Default keystore implementation for testing.
mod default_keystore;

pub use default_keystore::DefaultKeyStore as DefaultKeyStoreImpl;
use thiserror::Error;

// TODO: add windows implementation: https://learn.microsoft.com/en-us/windows/win32/seccng/key-storage-and-retrieval
// TODO: add iOS implementation: https://developer.apple.com/documentation/security/keychain_services

/// Errors that can occur when interacting with the keystore.
#[derive(Error, Debug)]
pub enum KeyStoreError {
    /// An error occurred during a cryptographic operation.
    #[error("Crypto failure")]
    CryptoError(String),
    /// The requested key was not found.
    #[error("Key not found in keystore")]
    KeyNotFound,
    /// An unexpected error occurred.
    #[error("Unexpected error occurred")]
    Unexpected,
}

/// This trait provides a generic interface for key management operations,
/// abstracting the underlying platform-specific keystore implementations (e.g.,
/// Android Keystore or a software-based keystore on other platforms).
///
/// All methods are static and operate on a fixed key alias.
pub trait KeyStore<C: CryptoProvider>: Send + Sync + 'static {
    /// Ensures a key exists, generating it if it doesn't.
    ///
    /// This function should be called before any cryptographic operations are
    /// performed to ensure the key is available. If a key
    /// with the alias does not exist, a new one will be generated and stored.
    ///
    /// # Returns
    ///
    /// A `Result` which is `Ok(())` on success or a `KeyStoreError` on failure.
    fn ensure_key_exists() -> Result<(), KeyStoreError>;

    /// Encrypts the given plaintext using the stored key.
    ///
    /// # Arguments
    ///
    /// * `plaintext` - The data to be encrypted.
    ///
    /// # Returns
    ///
    /// A `Result` containing the ciphertext as a `Vec<u8>` on success, or a
    /// `KeyStoreError` on failure.
    fn encrypt(plaintext: &[u8]) -> Result<Vec<u8>, KeyStoreError>;

    /// Decrypts the given ciphertext using the stored key.
    ///
    /// # Arguments
    ///
    /// * `ciphertext` - The data to be decrypted.
    ///
    /// # Returns
    ///
    /// A `Result` containing the plaintext as a `Vec<u8>` on success, or a
    /// `KeyStoreError` on failure.
    fn decrypt(ciphertext: &[u8]) -> Result<Vec<u8>, KeyStoreError>;

    /// Deletes the key from the keystore.
    ///
    /// # Returns
    ///
    /// A `Result` which is `Ok(())` on success or a `KeyStoreError` if the key
    /// could not be found or deleted.
    fn delete_key() -> Result<(), KeyStoreError>;
}
