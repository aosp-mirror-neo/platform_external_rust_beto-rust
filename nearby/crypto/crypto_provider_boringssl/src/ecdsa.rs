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

use crypto_provider::ecdsa::{
    InvalidPublicKeyBytes, P256EcdsaProvider, PRIVATE_KEY_LENGTH, PUBLIC_KEY_LENGTH,
    SIGNATURE_MAX_LENGTH,
};
use crypto_provider::tinyvec::ArrayVec;

pub struct P256Ecdsa;

impl P256EcdsaProvider for P256Ecdsa {
    type P256KeyPair = P256EcdsaKeyPair;

    type P256PublicKey = P256EcdsaPublicKey;

    type P256Signature = P256EcdsaSignature;
}

pub struct P256EcdsaKeyPair(bssl_crypto::ecdsa::PrivateKey<bssl_crypto::ec::P256>);

impl crypto_provider::ecdsa::KeyPair for P256EcdsaKeyPair {
    type P256PublicKey = P256EcdsaPublicKey;

    type P256Signature = P256EcdsaSignature;

    fn raw_private_key(
        &self,
        _permit: &crypto_provider::ecdsa::RawPrivateKeyPermit,
    ) -> crypto_provider::ecdsa::RawP256PrivateKey {
        let mut bytes = [0u8; PRIVATE_KEY_LENGTH];
        bytes.copy_from_slice(self.0.to_big_endian().as_ref());
        bytes
    }

    #[allow(clippy::unwrap_used)]
    fn from_raw_private_key(
        bytes: &crypto_provider::ecdsa::RawP256PrivateKey,
        _permit: &crypto_provider::ecdsa::RawPrivateKeyPermit,
    ) -> Self
    where
        Self: Sized,
    {
        // Unwrap is safe here because the private key bytes can be fully random
        // (no curve-related constraints), and when we instantiate the raw private
        // key, we check the size, so this should never fail.
        let inner_key =
            bssl_crypto::ecdsa::PrivateKey::<bssl_crypto::ec::P256>::from_big_endian(bytes)
                .unwrap();
        Self(inner_key)
    }

    #[allow(clippy::unwrap_used)]
    fn sign(&self, msg: &[u8]) -> Self::P256Signature {
        // Signing is an infallible operation, and we have a variable allocated length,
        // so this should be fine.
        P256EcdsaSignature(self.0.sign(msg).as_slice().try_into().unwrap())
    }

    fn generate() -> Self {
        let inner_key = bssl_crypto::ecdsa::PrivateKey::<bssl_crypto::ec::P256>::generate();
        Self(inner_key)
    }

    fn public_key(&self) -> Self::P256PublicKey {
        P256EcdsaPublicKey(self.0.to_public_key())
    }
}

pub struct P256EcdsaPublicKey(bssl_crypto::ecdsa::PublicKey<bssl_crypto::ec::P256>);

impl crypto_provider::ecdsa::PublicKey for P256EcdsaPublicKey {
    type Signature = P256EcdsaSignature;

    fn from_bytes(
        bytes: &crypto_provider::ecdsa::RawP256PublicKey,
    ) -> Result<Self, crypto_provider::ecdsa::InvalidPublicKeyBytes>
    where
        Self: Sized,
    {
        let inner_key =
            bssl_crypto::ecdsa::PublicKey::<bssl_crypto::ec::P256>::from_x962_uncompressed(bytes)
                .ok_or(InvalidPublicKeyBytes)?;
        Ok(Self(inner_key))
    }

    fn to_bytes(&self) -> crypto_provider::ecdsa::RawP256PublicKey {
        let mut bytes = [0u8; PUBLIC_KEY_LENGTH];
        bytes.copy_from_slice(self.0.to_x962_uncompressed().as_ref());
        bytes
    }

    fn verify_strict(
        &self,
        message: &[u8],
        signature: &Self::Signature,
    ) -> Result<(), crypto_provider::ecdsa::VerifyError> {
        self.0.verify(message, &signature.0).map_err(|_| crypto_provider::ecdsa::VerifyError)
    }
}

pub struct P256EcdsaSignature(ArrayVec<[u8; SIGNATURE_MAX_LENGTH]>);

impl crypto_provider::ecdsa::Signature for P256EcdsaSignature {
    fn from_bytes(
        bytes: &crypto_provider::ecdsa::RawP256Signature,
    ) -> Result<Self, crypto_provider::ecdsa::InvalidSignatureBytes> {
        Ok(Self(*bytes))
    }

    fn to_bytes(&self) -> crypto_provider::ecdsa::RawP256Signature {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::P256Ecdsa;
    use crypto_provider_test::ecdsa::*;

    #[test]
    fn p256_ecdsa_tests() {
        run_wycheproof_test_vectors::<P256Ecdsa>(EcdsaTestName::EcdsaSecp256r1Sha256);
    }
}
