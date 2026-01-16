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

use crate::common::{get_current_system_time, get_timestamp_claim_as_seconds};
use crate::intermediate_key_identifier::IntermediateKeyIdentifierError;
#[cfg(feature = "create")]
use crate::PrivateKey;
use crate::{common, KeyError, PublicKey, VerificationError};
use alloc::vec::Vec;
use coset::cbor::Value;
use coset::cwt::ClaimsSet;
use coset::{CborSerializable, CoseError};
use crypto_provider::CryptoProvider;
use thiserror::Error;

/// Size of an encrypted identity hash.
pub const ENCRYPTED_IDENTITY_HASH_SIZE: usize = 32;
// Custom claim IDs
pub(crate) const POLICY_CLAIM_ID: i64 = -11255476;
pub(crate) const PUBLIC_KEY_CLAIM_ID: i64 = -11255477;
pub(crate) const ENCRYPTED_IDENTITY_HASH_CLAIM_ID: i64 = -11255478;

/// An anonymous certificate signed by a trusted broker.
///
/// A TrustToken asserts that its policy has been verified against its subject
/// by the issuer.
#[derive(Debug)]
pub struct TrustToken {
    claims: ClaimsSet,
}

/// Error type for conversion failures.
#[derive(Debug)]
pub enum TrustTokenError {
    /// The CoseSign1 payload is missing.
    MissingPayload,
    /// Failed to deserialize the payload as a CWT claims set.
    CborDeserialization(CoseError),
    /// A required claim was missing.
    MissingClaim(TrustTokenClaim),
    /// A claim had an unexpected CBOR type or value.
    InvalidClaimFormat(TrustTokenClaim),
    /// The issuer had an invalid format.
    IntermediateIssuer(IntermediateKeyIdentifierError),
    /// The token was not signed by a trust anchor.
    SignatureError(VerificationError),
    /// The token is expired.
    Expired,
    /// Error if issued at time is invalid (in the future)
    InvalidIssuedAtTime,
}

/// Error type for challenge failures.
#[derive(Debug, Error)]
pub enum ChallengeError {
    /// Invalid key.
    #[error("invalid key")]
    InvalidKey(#[from] KeyError),
    /// The challenge response is wrong.
    #[error("the challenge response is wrong")]
    WrongResponse,
}

/// The claims defined within a TrustToken.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TrustTokenClaim {
    /// Identifies the principal that issued the TrustToken.
    Issuer,
    /// Identifies the expiration time on or after which the TrustToken MUST NOT be accepted.
    ExpirationTime,
    /// Identifies the time at which the TrustToken was issued.
    IssuedAt,
    /// Indicates the policy verified by the issuer.
    Policy,
    /// Contains the public key associated with the anonymous subject.
    PublicKey,
    /// Contains a hash related to the subject's identity, typically present
    /// for identity verification policies.
    EncryptedIdentityHash,
}

impl TrustToken {
    /// Parses a signed CWT encoding of a token, verifying it against the trust anchor.
    pub fn from_encoded<C: CryptoProvider>(
        encoded: coset::CoseSign1,
        trust_anchor: &crate::TrustAnchor,
    ) -> Result<Self, TrustTokenError> {
        trust_anchor
            .verify::<C>(encoded)
            .map_err(|err| match err {
                VerificationError::MissingPayload => TrustTokenError::MissingPayload,
                VerificationError::CborDeserialization(e) => {
                    TrustTokenError::CborDeserialization(e)
                }
                VerificationError::MissingIssuerClaim => {
                    TrustTokenError::MissingClaim(TrustTokenClaim::Issuer)
                }
                _ => TrustTokenError::SignatureError(err),
            })
            .and_then(|claims| {
                validate_claim_set(&claims)?;
                Ok(Self::from_claims(claims))
            })
    }

    /// Builds a token from a claim set without verifying the required fields are present. Note
    /// that a public key claim is still required.
    pub(crate) fn from_claims(claims: ClaimsSet) -> Self {
        Self { claims }
    }

    /// Attempts to convert a CoseSign1 structure into a TrustToken.
    ///
    /// This function assumes the payload of the CoseSign1 is a valid
    /// CBOR-encoded CWT ClaimsSet and verifies that all required claims
    /// are present. It does *not* verify the signature. Returns an error upon finding the first missing
    /// required claim.
    #[cfg(feature = "testing")]
    pub fn from_cose_sign1(cose: coset::CoseSign1) -> Result<Self, TrustTokenError> {
        use coset::CborSerializable;
        let payload_bytes = cose.payload.as_ref().ok_or(TrustTokenError::MissingPayload)?;
        let claims = ClaimsSet::from_slice(payload_bytes)
            .map_err(|e| TrustTokenError::CborDeserialization(e))?;
        Ok(Self::from_claims(claims))
    }

    /// Returns the issuer claim (iss).
    pub fn issuer(&self) -> &str {
        self.claims.issuer.as_deref().expect("This can only ever be created with an issuer present")
    }

    /// Returns the expiration time claim (exp).
    pub fn expiration_time(&self) -> Seconds {
        common::get_timestamp_claim_as_seconds(&self.claims.expiration_time)
            .expect("expiration time is a required field and this is verified upon token creation")
    }

    /// Returns the issued at claim (iat).
    pub fn issued_at(&self) -> Seconds {
        common::get_timestamp_claim_as_seconds(&self.claims.issued_at)
            .expect("issued at is a required field and this is verified upon token creation")
    }

    /// Returns the policy claim.
    pub fn policy(&self) -> TrustTokenPolicy {
        TrustTokenPolicy::try_from(
            common::get_private_claim_value(&self.claims.rest, POLICY_CLAIM_ID)
                .expect("policy is required and we verified its present upon creation"),
        )
        .expect("We verified the claim is in the correct policy format upon creation")
    }

    /// Returns the public key encoded bytes in CoseKey format.
    pub fn public_key_bytes(&self) -> &[u8] {
        match common::get_private_claim_value(&self.claims.rest, PUBLIC_KEY_CLAIM_ID)
            .expect("public key is a required field and we verified it exists upon creation")
        {
            Value::Bytes(b) => b,
            _ => unreachable!(
                "we validated the public key is the correct format upon token creation"
            ),
        }
    }

    /// Returns the encrypted identity hash claim.
    /// Returns `TrustTokenError::MissingClaim` if the claim is not present.
    pub fn encrypted_identity_hash(&self) -> Result<Vec<u8>, TrustTokenError> {
        let value =
            common::get_private_claim_value(&self.claims.rest, ENCRYPTED_IDENTITY_HASH_CLAIM_ID)
                .ok_or(TrustTokenError::MissingClaim(TrustTokenClaim::EncryptedIdentityHash))?;

        match value {
            Value::Bytes(b) => Ok(b.clone()),
            _ => Err(TrustTokenError::InvalidClaimFormat(TrustTokenClaim::EncryptedIdentityHash)),
        }
    }

    /// Verifies the challenge sent by a peer.
    pub fn verify_challenge<C: CryptoProvider>(
        &self,
        challenge: &[u8],
        remote_response: &[u8],
    ) -> Result<(), ChallengeError> {
        let public_key = coset::CoseKey::from_slice(self.public_key_bytes())
            .map_err(|e| ChallengeError::InvalidKey(KeyError::CborDeserialization(e)))
            .and_then(|key| PublicKey::from_cose_key::<C>(&key).map_err(ChallengeError::from))?;
        public_key
            .verify::<C>(challenge, remote_response)
            .map_err(|_| ChallengeError::WrongResponse)
    }

    /// Signs the TrustToken with the signing key and returns its CoseSign1 representation.
    #[cfg(feature = "create")]
    pub fn sign<C: CryptoProvider>(
        &self,
        signing_key: &PrivateKey,
    ) -> Result<coset::CoseSign1, CoseError> {
        use coset::{iana, CborSerializable};

        // TODO: Verify this is what we want here.
        let protected = coset::HeaderBuilder::new().algorithm(iana::Algorithm::EdDSA).build();
        let unprotected = coset::HeaderBuilder::new().key_id(b"AsymmetricEdDSA".to_vec()).build();
        let aad = b"";

        Ok(coset::CoseSign1Builder::new()
            .protected(protected)
            .unprotected(unprotected)
            .payload(self.claims.clone().to_vec()?)
            .create_signature(aad, |pt| signing_key.sign::<C>(pt))
            .build())
    }
}

pub(crate) fn validate_claim_set(claims: &ClaimsSet) -> Result<(), TrustTokenError> {
    if claims.issuer.is_none() {
        return Err(TrustTokenError::MissingClaim(TrustTokenClaim::Issuer));
    }

    let current_time = get_current_system_time();

    get_timestamp_claim_as_seconds(&claims.expiration_time)
        .map(|s| if current_time >= s { Err(TrustTokenError::Expired) } else { Ok(()) })
        .unwrap_or(Err(TrustTokenError::MissingClaim(TrustTokenClaim::ExpirationTime)))?;

    get_timestamp_claim_as_seconds(&claims.issued_at)
        .map(|s| if s > current_time { Err(TrustTokenError::InvalidIssuedAtTime) } else { Ok(()) })
        .unwrap_or(Err(TrustTokenError::MissingClaim(TrustTokenClaim::IssuedAt)))?;

    let policy = TrustTokenPolicy::try_from(
        common::get_private_claim_value(&claims.rest, POLICY_CLAIM_ID)
            .ok_or(TrustTokenError::MissingClaim(TrustTokenClaim::Policy))?,
    )?;

    if matches!(
        policy,
        TrustTokenPolicy::VerifiedIdentity | TrustTokenPolicy::VerifiedIdentityStrong
    ) {
        check_claim_is_bytes(
            claims,
            TrustTokenClaim::EncryptedIdentityHash,
            ENCRYPTED_IDENTITY_HASH_CLAIM_ID,
        )?;
    }

    check_claim_is_bytes(claims, TrustTokenClaim::PublicKey, PUBLIC_KEY_CLAIM_ID)?;

    Ok(())
}

fn check_claim_is_bytes(
    claims: &ClaimsSet,
    claim: TrustTokenClaim,
    claim_id: i64,
) -> Result<(), TrustTokenError> {
    match common::get_private_claim_value(&claims.rest, claim_id)
        .ok_or(TrustTokenError::MissingClaim(claim.clone()))?
    {
        Value::Bytes(_) => Ok(()),
        _ => Err(TrustTokenError::InvalidClaimFormat(claim)),
    }
}

/// The output of a policy function `f(uid, attr)` about a user or their device
/// (`uid`) and attributes (`attr`) about either.
/// Policies:
/// * Are issued by a validation service based upon evaluation of the policy function.
/// * Are used by relying parties to make decisions when interacting with the device holding the token.
/// * Contain no PII
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u8)]
pub enum TrustTokenPolicy {
    /// The identity attribute associated with an encrypted identity hash has
    /// been verified.
    VerifiedIdentity = 1,
    /// Like `VerifiedIdentity`, but with stronger requirements:
    ///
    /// *   The associated public key is explicitly bound to the identity attribute.
    /// *   There is no known way to spoof the identity without compromising the client device.
    VerifiedIdentityStrong = 2,
    /// A device attestation has been verified.
    VerifiedDevice = 3,
    /// Like `VerifiedDevice`, but with stronger requirements:
    ///
    /// *   The associated public key is explicitly bound to the attestation.
    /// *   The attestation includes information about the client application.
    /// *   There is no known way to spoof the attestation without compromising a device
    ///     hardware-backed key.
    VerifiedDeviceStrong = 4,
}

impl TryFrom<&Value> for TrustTokenPolicy {
    type Error = TrustTokenError;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        match value {
            Value::Integer(val) => {
                // Convert the CBOR integer (i64/u64) to u8
                let byte_val: u8 = (*val)
                    .try_into()
                    // If conversion fails (e.g., out of range), it's an invalid format
                    .map_err(|_| TrustTokenError::InvalidClaimFormat(TrustTokenClaim::Policy))?;

                // Map the u8 to the enum variant
                match byte_val {
                    1 => Ok(TrustTokenPolicy::VerifiedIdentity),
                    2 => Ok(TrustTokenPolicy::VerifiedIdentityStrong),
                    3 => Ok(TrustTokenPolicy::VerifiedDevice),
                    4 => Ok(TrustTokenPolicy::VerifiedDeviceStrong),
                    // Any other u8 value is invalid
                    _ => Err(TrustTokenError::InvalidClaimFormat(TrustTokenClaim::Policy)),
                }
            }
            // If the CBOR value is not an integer, it's an invalid format
            _ => Err(TrustTokenError::InvalidClaimFormat(TrustTokenClaim::Policy)),
        }
    }
}

/// A NumericDate value as defined in RFC 7519. Number of seconds since the Unix epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Seconds(pub u32);

/// The encrypted identity hash of the subject.
pub struct EncryptedIdentityHash(pub [u8; ENCRYPTED_IDENTITY_HASH_SIZE]);
