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

use std::vec::Vec;

use coset::{
    cbor::{self, Value},
    iana, CborSerializable, CoseError, CoseKey, CoseKeyBuilder,
};
#[cfg(feature = "create")]
use crypto_provider::ed25519::RawPrivateKeyPermit;
use crypto_provider::{ed25519, CryptoProvider};
use thiserror::Error;

/// An abstraction over the public used by [TrustAnchor] and [TrustToken].
#[derive(Clone, Debug, PartialEq)]
pub struct PublicKey(ed25519::PublicKey);

/// Key errors.
#[derive(Debug, Error)]
pub enum KeyError {
    /// Failed to deserialize the key from a COSE_KEY.
    #[error("failed to deserialize the key from a COSE_KEY")]
    CborDeserialization(CoseError),
    /// Failed to serailize the key into a COSE_KEY.
    #[error("failed to serialize the key into a COSE_KEY")]
    CborSerialization(CoseError),
    /// Invalid key format.
    #[error("invalid key format")]
    InvalidFormat,
}

/// Error type for challenge failures.
#[derive(Debug, Error)]
#[error("invalid signature")]
pub struct SignatureError;

impl PublicKey {
    /// Constructs the public key from a CWT claim.
    /// TODO: move to a ClaimsSet wrapper.
    pub(crate) fn from_claim<C: CryptoProvider>(value: &Value) -> Result<PublicKey, KeyError> {
        let Value::Bytes(cose_key_bytes) = value else {
            return Err(KeyError::InvalidFormat);
        };
        coset::CoseKey::from_slice(cose_key_bytes)
            .map_err(|e| KeyError::CborDeserialization(e))
            .and_then(|key| Self::from_cose_key::<C>(&key))
    }

    /// Parses the public key from raw key bytes.
    pub fn from_bytes<C: CryptoProvider>(bytes: &[u8]) -> Result<PublicKey, KeyError> {
        ed25519::PublicKey::from_bytes::<C::Ed25519>(
            bytes.try_into().map_err(|_| KeyError::InvalidFormat)?,
        )
        .map(|key| PublicKey(key))
        .map_err(|_| KeyError::InvalidFormat)
    }

    /// Consumes the public key and returns raw key bytes.
    pub fn to_bytes(self) -> Vec<u8> {
        self.0.into_bytes().to_vec()
    }

    /// Constructs the public key from a Cose_Key.
    pub fn from_cose_key<C: CryptoProvider>(key: &CoseKey) -> Result<PublicKey, KeyError> {
        // Since HPKE key representation for COSE has not been standardized, an encryption key for
        // use with the Tink scheme DHKEM_X25519_HKDF_SHA256_HKDF_SHA256_AES_128_GCM is represented
        // as an EC2 public key with the alg and key_ops fields omitted.
        if key.kty != coset::KeyType::Assigned(iana::KeyType::EC2)
            || key.alg.is_some()
            || !key.key_ops.is_empty()
        {
            return Err(KeyError::InvalidFormat);
        }

        if !key.params.iter().any(|(label, value)| {
            label == &coset::Label::Int(iana::Ec2KeyParameter::Crv as i64)
                && value == &cbor::Value::from(iana::EllipticCurve::Ed25519 as u64)
        }) {
            return Err(KeyError::InvalidFormat);
        }

        let x = key
            .params
            .iter()
            .find_map(|(label, value)| match (label, value) {
                (&coset::Label::Int(int_label), cbor::Value::Bytes(bytes))
                    if int_label == iana::Ec2KeyParameter::X as i64 =>
                {
                    bytes.clone().try_into().ok()
                }
                _ => None,
            })
            .ok_or(KeyError::InvalidFormat)?;

        ed25519::PublicKey::from_bytes::<C::Ed25519>(x)
            .map_err(|_| KeyError::InvalidFormat)
            .map(|key| Self(key))
    }

    pub(crate) fn verify<C: CryptoProvider>(
        &self,
        message: &[u8],
        signature: &[u8],
    ) -> Result<(), SignatureError> {
        let signature =
            signature.try_into().map(ed25519::Signature::new).map_err(|_| SignatureError)?;
        self.0.verify_strict::<C::Ed25519>(message, signature).map_err(|_| SignatureError)
    }
}

impl From<PublicKey> for CoseKey {
    fn from(key: PublicKey) -> Self {
        CoseKeyBuilder::new()
            .key_type(iana::KeyType::EC2)
            .param(
                iana::Ec2KeyParameter::Crv as i64,
                Value::from(iana::EllipticCurve::Ed25519 as u64),
            )
            .param(iana::Ec2KeyParameter::X as i64, Value::Bytes(key.0.into_bytes().to_vec()))
            .build()
    }
}

impl TryFrom<PublicKey> for Value {
    type Error = KeyError;

    /// Encodes the public key to a CWT claim.
    fn try_from(key: PublicKey) -> Result<Self, Self::Error> {
        let cose_key_bytes =
            CoseKey::from(key).to_vec().map_err(|e| KeyError::CborSerialization(e))?;
        Ok(Value::Bytes(cose_key_bytes))
    }
}

/// An abstraction over the private key used by [TrustAnchor] and [TrustToken].
#[cfg(feature = "create")]
#[derive(Clone)]
pub struct PrivateKey(ed25519::PrivateKey);

#[cfg(feature = "create")]
impl PrivateKey {
    /// Constructs the private key from raw key bytes.
    pub fn from_bytes(bytes: &[u8], permit: &RawPrivateKeyPermit) -> Result<PrivateKey, KeyError> {
        Ok(PrivateKey(ed25519::PrivateKey::from_raw_private_key(
            bytes.try_into().map_err(|_| KeyError::InvalidFormat)?,
            permit,
        )))
    }

    /// Converts the private key to raw bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        use crypto_provider::ed25519::RawPrivateKeyPermit;

        self.0.raw_private_key(&RawPrivateKeyPermit::default()).to_vec()
    }

    /// Signs the message.
    pub fn sign<C: CryptoProvider>(&self, message: &[u8]) -> Vec<u8> {
        self.0.sign::<C::Ed25519>(message).to_bytes().to_vec()
    }
}

/// Generatea a new key pair.
#[cfg(feature = "create")]
pub fn generate_key<C: CryptoProvider>() -> (PublicKey, PrivateKey) {
    let private_key = PrivateKey(ed25519::PrivateKey::generate::<C::Ed25519>());
    let public_key = PublicKey(private_key.0.derive_public_key::<C::Ed25519>());
    return (public_key, private_key);
}

#[cfg(test)]
mod test {
    use super::*;
    use crypto_provider_default::CryptoProviderImpl;

    #[test]
    fn test_public_key_conversion() -> Result<(), KeyError> {
        let (public_key, _) = generate_key::<CryptoProviderImpl>();
        let public_key_bytes = Value::try_from(public_key.clone())?;

        assert_eq!(
            public_key,
            PublicKey::from_claim::<CryptoProviderImpl>(&public_key.clone().try_into()?)?
        );
        assert_eq!(
            public_key_bytes,
            Value::try_from(PublicKey::from_claim::<CryptoProviderImpl>(&public_key_bytes)?)?
        );
        assert_eq!(
            public_key,
            PublicKey::from_cose_key::<CryptoProviderImpl>(&public_key.clone().into())?
        );
        assert_eq!(
            public_key,
            PublicKey::from_bytes::<CryptoProviderImpl>(&public_key.clone().to_bytes())?
        );
        Ok(())
    }
}
