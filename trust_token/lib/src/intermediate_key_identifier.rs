// Copyright 2025 Google LLC
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

use alloc::format;
use alloc::string::{String, ToString};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use core::convert::TryFrom;
use core::fmt;
use coset::{CborSerializable, CoseError, CoseKey};
use crypto_provider::sha2::Sha256;
use std::vec::Vec;

/// Represents the key identifier for an intermediate authority.
///
/// The identifier is derived from the authority's public key (in COSE_Key format) by taking the
/// SHA-256 hash and extracting the last 9 bytes of the hash.
///
/// Derivation Formula: `ID = SHA256(cose_key)[-9:]`
///
/// The canonical string representation, implemented in the `Display` trait, encodes these 9 bytes
/// using Base64-URL encoding (without padding). The resulting value is typically a 12-character
/// string.
///
/// This identifier is used in the `sub` (Subject) claim of an intermediate certificate and the
/// `iss` (Issuer) claim of a Talisman issued by that intermediate.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IntermediateKeyIdentifier([u8; 9]);

impl IntermediateKeyIdentifier {
    /// Creates an identifier.
    pub fn new(hash: [u8; 9]) -> Self {
        Self(hash)
    }

    /// Creates an identifier from a pre-encoded 12-character Base64-URL string.
    ///
    /// This is used when the identifier is already known, for example, when parsing
    /// the `iss` claim from an existing Talisman.
    ///
    /// # Errors
    ///
    /// Returns `IntermediateKeyIdentifierError::InvalidEncodedLength` if the input
    /// string's length is not exactly 12.
    pub fn try_from_encoded_string(
        encoded_identifier: String,
    ) -> Result<Self, IntermediateKeyIdentifierError> {
        Ok(Self(
            URL_SAFE_NO_PAD
                .decode(encoded_identifier)
                .map_err(|_| {
                    IntermediateKeyIdentifierError::DerivationError(
                        "Invalid base64 encoding".into(),
                    )
                })?
                .try_into()
                .map_err(|e: Vec<u8>| {
                    IntermediateKeyIdentifierError::InvalidEncodedLength(e.len())
                })?,
        ))
    }

    /// Derives the identifier from an intermediate authority's public key (COSE_Key format).
    ///
    /// This follows the formula: `ID = SHA256(cose_key)[-9:]`
    ///
    /// This is used when constructing identifiers, for example, when creating the `sub` claim for a
    /// new intermediate certificate or the `iss` claim for a Talisman being issued by a known
    /// intermediate key.
    ///
    /// # Errors
    ///
    /// Returns `IntermediateKeyIdentifierError::DerivationError` if the `CoseKey` cannot be
    /// serialized.
    pub fn try_from_cose_key<S: Sha256>(
        authority_public_key: CoseKey,
    ) -> Result<Self, IntermediateKeyIdentifierError> {
        let key_bytes = authority_public_key.to_vec().map_err(|e: CoseError| {
            IntermediateKeyIdentifierError::DerivationError(format!(
                "COSE Key serialization failed: {e}"
            ))
        })?;

        let hash_result = S::sha256(&key_bytes);
        Ok(Self(hash_result[23..].try_into().expect("the hash result should be 32 bytes")))
    }
}

/// Enables easy display and formatting of the identifier.
impl fmt::Display for IntermediateKeyIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", URL_SAFE_NO_PAD.encode(self.0))
    }
}

/// Allows attempting to create an identifier from a `String` (assumes pre-encoded).
impl TryFrom<String> for IntermediateKeyIdentifier {
    type Error = IntermediateKeyIdentifierError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        IntermediateKeyIdentifier::try_from_encoded_string(s)
    }
}

/// Allows attempting to create an identifier from a `&str` (assumes pre-encoded).
impl TryFrom<&str> for IntermediateKeyIdentifier {
    type Error = IntermediateKeyIdentifierError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        // Clone the string slice into an owned String for the struct
        IntermediateKeyIdentifier::try_from_encoded_string(s.to_string())
    }
}

/// Error type for IntermediateKeyIdentifier creation or derivation failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntermediateKeyIdentifierError {
    /// The provided pre-encoded string representation has an invalid length (must be 12 characters).
    InvalidEncodedLength(usize),
    /// An error occurred during the derivation process from a CoseKey (e.g., serialization).
    DerivationError(String),
}

impl fmt::Display for IntermediateKeyIdentifierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IntermediateKeyIdentifierError::InvalidEncodedLength(len) => {
                write!(
                    f,
                    "invalid pre-encoded intermediate key identifier length: expected 12, got {}",
                    len
                )
            }
            IntermediateKeyIdentifierError::DerivationError(msg) => {
                write!(f, "failed to derive intermediate key identifier: {}", msg)
            }
        }
    }
}
