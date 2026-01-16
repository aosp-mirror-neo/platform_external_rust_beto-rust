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

use std::string::ToString as _;

use crate::trust_token::{
    EncryptedIdentityHash, Seconds, TrustToken, TrustTokenError, TrustTokenPolicy,
    ENCRYPTED_IDENTITY_HASH_CLAIM_ID, POLICY_CLAIM_ID, PUBLIC_KEY_CLAIM_ID,
};
use crate::{validate_claim_set, IntermediateKeyIdentifier, PublicKey, TrustTokenClaim};
use coset::cwt;
use crypto_provider::CryptoProvider;

/// Builder used for creating a [TrustToken].
pub struct TrustTokenBuilder {
    claims_builder: cwt::ClaimsSetBuilder,
    public_key: Option<PublicKey>,
    issuer: Option<IntermediateKeyIdentifier>,
}

impl TrustTokenBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self { claims_builder: cwt::ClaimsSetBuilder::new(), public_key: None, issuer: None }
    }

    /// Builds the unsigned token with the set claims.
    pub fn build<C: CryptoProvider>(self) -> Result<TrustToken, TrustTokenError> {
        let claims = self
            .claims_builder
            .issuer(
                self.issuer
                    .ok_or(TrustTokenError::MissingClaim(TrustTokenClaim::Issuer))?
                    .to_string(),
            )
            .private_claim(
                PUBLIC_KEY_CLAIM_ID,
                self.public_key
                    .ok_or(TrustTokenError::MissingClaim(TrustTokenClaim::PublicKey))?
                    .try_into()
                    .map_err(|_| TrustTokenError::InvalidClaimFormat(TrustTokenClaim::PublicKey))?,
            )
            .build();
        validate_claim_set(&claims)?;
        Ok(TrustToken::from_claims(claims))
    }

    /// Builds the unsigned token with the set claims, without verifying that the required fields
    /// are present. USED ONLY FOR TESTING PURPOSES.
    #[cfg(feature = "testing")]
    pub fn build_for_test(self) -> TrustToken {
        use crate::key::generate_key;
        use crypto_provider_default::CryptoProviderImpl;

        // Generates a public key if one is not provided.
        let public_key = self.public_key.unwrap_or_else(|| generate_key::<CryptoProviderImpl>().0);
        let claims = self
            .claims_builder
            .issuer(self.issuer.expect("issuer is required").to_string())
            .private_claim(
                PUBLIC_KEY_CLAIM_ID,
                public_key.try_into().expect("public key should be valid"),
            )
            .build();
        TrustToken::from_claims(claims)
    }

    /// The issuing principle.
    #[must_use]
    pub fn issuer(mut self, issuer: IntermediateKeyIdentifier) -> Self {
        self.issuer = Some(issuer);
        self
    }

    /// The expiration.
    #[must_use]
    pub fn expiration_time(mut self, expiration_time: Seconds) -> Self {
        self.claims_builder = self
            .claims_builder
            .expiration_time(cwt::Timestamp::WholeSeconds(expiration_time.0.into()));
        self
    }

    /// The time issued.
    #[must_use]
    pub fn issued_at(mut self, issued_at: Seconds) -> Self {
        self.claims_builder =
            self.claims_builder.issued_at(cwt::Timestamp::WholeSeconds(issued_at.0.into()));
        self
    }

    /// The policy used to verify the identity
    #[must_use]
    pub fn policy(mut self, policy: TrustTokenPolicy) -> Self {
        self.claims_builder =
            self.claims_builder.private_claim(POLICY_CLAIM_ID, (policy as u8).into());
        self
    }

    /// The public key associated with the anonymous subject.
    #[must_use]
    pub fn public_key(mut self, public_key: PublicKey) -> Self {
        self.public_key = Some(public_key);
        self
    }

    /// For [TrustTokenPolicy::VerifiedIdentity] and [TrustTokenPolicy::VerifiedIdentityStrong]: the
    /// encrypted identity hash.
    #[must_use]
    pub fn encrypted_identity_hash(
        mut self,
        encrypted_identity_hash: EncryptedIdentityHash,
    ) -> Self {
        self.claims_builder = self.claims_builder.private_claim(
            ENCRYPTED_IDENTITY_HASH_CLAIM_ID,
            encrypted_identity_hash.0.as_slice().into(),
        );
        self
    }
}
