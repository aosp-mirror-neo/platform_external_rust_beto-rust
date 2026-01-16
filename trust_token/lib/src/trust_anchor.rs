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

use crate::common::get_current_system_time;
use crate::{common, PublicKey};
use crate::{Seconds, SignatureError};
use alloc::string::String;
use alloc::vec::Vec;
use coset::{cbor, cwt, CborSerializable, CoseError, CoseSign1};
use crypto_provider::CryptoProvider;

const INTERMEDIATE_PUBLIC_KEY_CLAIM_ID: i64 = -11255477;
const INTERMEDIATE_USAGE_CLAIM_ID: i64 = -11255479;

/// A set of verified intermediate certificates which can be used to verify
/// the signature of a TrustToken.
#[derive(Debug, Clone, Default)]
pub struct TrustAnchor {
    intermediates: Vec<IntermediateCertificate>,
}

/// Represents a verified intermediate certificate containing its claims.
#[derive(Debug, Clone)]

struct IntermediateCertificate {
    // TODO: remove
    #[allow(unused)]
    /// Standard Issuer claim (iss, 1). Key identifier of the issuing root.
    issuer: String,
    /// Standard Subject claim (sub, 2). Key identifier for this intermediate.
    subject: String,
    /// Standard Expiration time claim (exp, 4).
    expiration_time: Seconds,
    /// Standard Issued at claim (iat, 6).
    issued_at: Seconds,
    /// Custom Usage claim (-11255479). Indicates key usage (Signing or Encryption).
    usage: IntermediateUsage,
    /// Verifying key extracted from the public_key
    verifying_key: PublicKey,
}

/// The claims defined within a TrustToken.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum IntermediateClaim {
    /// Identifies the principal that issued the token.
    Issuer,
    /// The subject principle. The value is a key identifier which also appears in the `iss` field of every token this authority issues.
    Subject,
    /// The expiration, as a NumericDate value.
    ExpirationTime,
    /// The time of issuance, as a NumericDate value.
    IssuedAt,
    /// The associated authority public key, encoded as a COSE_Key. This key can either be a signing key or an encryption key, as indicated by the usage field. Since HPKE key representation for COSE has not been standardized, an encryption key for use with the Tink scheme DHKEM_X25519_HKDF_SHA256_HKDF_SHA256_AES_128_GCM is represented as an EC2 public key with the alg and key_ops fields omitted.
    PublicKey,
    /// Indicates what the key is to be used for. See [IntermediateUsage].
    Usage,
}

/// Indicates the intended usage of the intermediate certificate's public key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)] // Use u64 to match CBOR integer representation
pub enum IntermediateUsage {
    /// Key is for verifying token signatures (value 1).
    Signing = 1,
    /// Key is for encrypting client parameters (value 2).
    Encryption = 2,
}

/// Error types for TrustAnchor operations
#[derive(Debug)]
pub enum TrustAnchorError {
    /// The CoseSign1 payload is missing.
    MissingPayload,
    /// Failed to deserialize the payload as a CWT claims set.
    CborDeserialization(CoseError),
    /// A required claim was missing.
    MissingClaim(IntermediateClaim),
    /// A claim had an unexpected format.
    InvalidClaimFormat(IntermediateClaim),
    /// The intermediate certificate was not signed by a trust anchor.
    SignatureError,
    /// Multiple errors occurred
    ErrorSet(Vec<TrustAnchorError>),
    /// A problem with the root key
    RootKeyInvalid,
    /// Verification of a token against the trust anchor failed
    VerificationFailed(VerificationError),
}

/// Error types verification
#[derive(Debug)]
pub enum VerificationError {
    /// No intermediate with a subject that matches the token's issuer was found.
    IntermediateNotFound,
    /// The signature in the token had an invalid format.
    InvalidSignatureFormat,
    /// The crypto verification failed
    VerificationFailed(SignatureError),
    /// The intermediate certificate is not used for signatures.
    IncorrectKeyUsage,
    /// The token has a missing payload.
    MissingPayload,
    /// The token's payload could not be deserialized.
    CborDeserialization(CoseError),
    /// The token has a missing issuer claim, so we cannot determine which certificate to use.
    MissingIssuerClaim,
    /// The current time has passed the expiration time of the cert.
    ExpiredCertificate,
    /// The issued at time is in the future which is invalid.
    InvalidIssuedAtTime,
}

impl TryFrom<u64> for IntermediateUsage {
    type Error = TrustAnchorError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(IntermediateUsage::Signing),
            2 => Ok(IntermediateUsage::Encryption),
            _ => Err(TrustAnchorError::InvalidClaimFormat(IntermediateClaim::Usage)),
        }
    }
}

impl TrustAnchor {
    /// Construct a new TrustAnchor using only the intermediate certificates which are signed by one of the roots.
    pub fn new<C: CryptoProvider>(
        roots: &coset::CoseKeySet,
        intermediate_certificates: Vec<CoseSign1>,
    ) -> Result<Self, TrustAnchorError> {
        let root_verifying_keys: Vec<PublicKey> = roots
            .0 // Access the Vec<CoseKey> within CoseKeySet
            .iter()
            .filter_map(|root_key| PublicKey::from_cose_key::<C>(root_key).ok())
            .collect();

        // Helper function to process a single intermediate certificate
        let process_intermediate =
            |intermediate: CoseSign1| -> Result<IntermediateCertificate, TrustAnchorError> {
                // 1. Verify signature against roots
                if !Self::is_verified_by_any_root::<C>(&root_verifying_keys, &intermediate) {
                    return Err(TrustAnchorError::SignatureError);
                }

                // 2. Extract and deserialize payload
                let payload_bytes = intermediate.payload.ok_or(TrustAnchorError::MissingPayload)?;
                let claims = cwt::ClaimsSet::from_slice(&payload_bytes)
                    .map_err(|e| TrustAnchorError::CborDeserialization(e))?;

                // 3. Create IntermediateCertificate from claims
                IntermediateCertificate::new::<C>(claims) // This returns Result<_, TrustAnchorError>
            };

        // Process all intermediates and collect results
        let results: Vec<Result<IntermediateCertificate, TrustAnchorError>> =
            intermediate_certificates.into_iter().map(process_intermediate).collect();

        // Separate successful results from errors
        let mut verified_intermediates = Vec::new();
        let mut errors = Vec::new();
        for result in results {
            match result {
                Ok(cert) => verified_intermediates.push(cert),
                Err(err) => errors.push(err),
            }
        }

        // Check if any errors occurred
        if !errors.is_empty() {
            Err(TrustAnchorError::ErrorSet(errors))
        } else {
            Ok(Self { intermediates: verified_intermediates })
        }
    }

    /// Verifies the token was signed by a configured trust anchor.
    pub fn verify<C: CryptoProvider>(
        &self,
        encoded_token: CoseSign1,
    ) -> Result<cwt::ClaimsSet, VerificationError> {
        let payload_bytes =
            encoded_token.payload.as_ref().ok_or(VerificationError::MissingPayload)?;

        let claims = cwt::ClaimsSet::from_slice(payload_bytes)
            .map_err(|e| VerificationError::CborDeserialization(e))?;

        let issuer = claims.issuer.as_ref().ok_or(VerificationError::MissingIssuerClaim)?;

        let intermediate_cert = self
            .intermediates
            .iter()
            .find(|cert| cert.subject.eq(issuer))
            .ok_or(VerificationError::IntermediateNotFound)?;

        intermediate_cert.verify::<C>(encoded_token)?;

        Ok(claims)
    }

    fn is_verified_by_any_root<C>(
        root_verifying_keys: &Vec<PublicKey>,
        intermediate: &CoseSign1,
    ) -> bool
    where
        C: CryptoProvider,
    {
        root_verifying_keys.iter().any(|public_key| {
            let verifier = |sig_bytes: &[u8], tbs_data: &[u8]| -> Result<(), TrustAnchorError> {
                public_key.verify::<C>(tbs_data, sig_bytes)?;
                Ok(())
            };

            let external_aad: &[u8] = b""; // Assuming empty AAD
                                           // Pass a reference to the inline closure to verify_signature
            intermediate.verify_signature(external_aad, &verifier).is_ok()
        })
    }

    /// Returns the number of trust anchor certificates.
    pub fn size(&self) -> usize {
        self.intermediates.len()
    }
}

impl IntermediateCertificate {
    /// Creates a new IntermediateCertificate by extracting required claims.
    /// Returns an error if any required claim is missing or invalid.
    pub(crate) fn new<C>(claims: cwt::ClaimsSet) -> Result<Self, TrustAnchorError>
    where
        C: CryptoProvider,
    {
        let issuer =
            claims.issuer.ok_or(TrustAnchorError::MissingClaim(IntermediateClaim::Issuer))?;
        let subject =
            claims.subject.ok_or(TrustAnchorError::MissingClaim(IntermediateClaim::Subject))?;

        let expiration_time = common::get_timestamp_claim_as_seconds(&claims.expiration_time)
            .ok_or(TrustAnchorError::MissingClaim(IntermediateClaim::ExpirationTime))?;
        let issued_at = common::get_timestamp_claim_as_seconds(&claims.issued_at)
            .ok_or(TrustAnchorError::MissingClaim(IntermediateClaim::IssuedAt))?;

        // Extract Public Key
        let public_key_value =
            common::get_private_claim_value(&claims.rest, INTERMEDIATE_PUBLIC_KEY_CLAIM_ID)
                .ok_or(TrustAnchorError::MissingClaim(IntermediateClaim::PublicKey))?;
        let verifying_key = PublicKey::from_claim::<C>(&public_key_value)
            .map_err(|_| TrustAnchorError::InvalidClaimFormat(IntermediateClaim::PublicKey))?;

        // Extract Usage
        let usage_value =
            common::get_private_claim_value(&claims.rest, INTERMEDIATE_USAGE_CLAIM_ID)
                .ok_or(TrustAnchorError::MissingClaim(IntermediateClaim::Usage))?;
        let usage_uint = match usage_value {
            cbor::Value::Integer(ival) => u64::try_from(*ival)
                .map_err(|_| TrustAnchorError::InvalidClaimFormat(IntermediateClaim::Usage))?,
            _ => return Err(TrustAnchorError::InvalidClaimFormat(IntermediateClaim::Usage)),
        };
        let usage = IntermediateUsage::try_from(usage_uint)?;

        Ok(Self { issuer, subject, expiration_time, issued_at, verifying_key, usage })
    }

    fn verify<C>(&self, token: CoseSign1) -> Result<(), VerificationError>
    where
        C: CryptoProvider,
    {
        if self.usage != IntermediateUsage::Signing {
            return Err(VerificationError::IncorrectKeyUsage);
        }

        let current_time = get_current_system_time();
        if current_time >= self.expiration_time {
            return Err(VerificationError::ExpiredCertificate);
        }
        if self.issued_at > current_time {
            return Err(VerificationError::InvalidIssuedAtTime);
        }

        let verifier = |sig_bytes: &[u8], tbs_data: &[u8]| -> Result<(), VerificationError> {
            self.verifying_key
                .verify::<C>(tbs_data, sig_bytes)
                .map_err(|e| VerificationError::VerificationFailed(e))
        };

        let external_aad = b"";

        token.verify_signature(external_aad, &verifier)
    }
}

impl From<CoseError> for TrustAnchorError {
    fn from(err: CoseError) -> Self {
        TrustAnchorError::CborDeserialization(err)
    }
}

impl From<SignatureError> for TrustAnchorError {
    fn from(_: SignatureError) -> Self {
        TrustAnchorError::SignatureError
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::key::generate_key;
    use crate::{IntermediateKeyIdentifier, TrustTokenBuilder, TrustTokenPolicy};
    use crypto_provider_default::CryptoProviderImpl;

    #[test]
    fn intermediate_verify_expired_cert() {
        let (public_key, _) = generate_key::<CryptoProviderImpl>();
        let expired_cert = IntermediateCertificate {
            issuer: "abcd".into(),
            subject: "abcd".into(),
            expiration_time: Seconds(3),
            issued_at: Seconds(1),
            usage: IntermediateUsage::Signing,
            verifying_key: public_key,
        };

        assert!(matches!(
            expired_cert.verify::<CryptoProviderImpl>(default_token()).unwrap_err(),
            VerificationError::ExpiredCertificate
        ))
    }

    #[test]
    fn intermediate_verify_invalid_issued_at_cert() {
        let (public_key, _) = generate_key::<CryptoProviderImpl>();
        let expired_cert = IntermediateCertificate {
            issuer: "abcd".into(),
            subject: "abcd".into(),
            expiration_time: Seconds(u32::MAX),
            issued_at: Seconds(u32::MAX),
            usage: IntermediateUsage::Signing,
            verifying_key: public_key,
        };

        assert!(matches!(
            expired_cert.verify::<CryptoProviderImpl>(default_token()).unwrap_err(),
            VerificationError::InvalidIssuedAtTime
        ))
    }

    fn default_token() -> CoseSign1 {
        let (public_key, private_key) = generate_key::<CryptoProviderImpl>();
        let token = TrustTokenBuilder::new()
            .issuer(IntermediateKeyIdentifier::new([0, 1, 2, 3, 4, 5, 6, 7, 8]))
            .issued_at(Seconds(1))
            .expiration_time(Seconds(u32::MAX))
            .public_key(public_key)
            .policy(TrustTokenPolicy::VerifiedDevice)
            .build_for_test();
        token.sign::<CryptoProviderImpl>(&private_key).unwrap()
    }
}
