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

use storage::StorageError;
use thiserror::Error;

/// Encoding with visibility error type for identity manager.
#[derive(Error, Debug)]
pub enum IdentityManagerError {
    /// Storage error.
    #[error("Storage error")]
    StorageError(#[from] StorageError),
    /// Invalid device ID length.
    #[error("Invalid device ID length: {0}")]
    InvalidDeviceIdLength(usize),
    /// Generic error.
    #[error("Generic error")]
    GenericError,
    /// Channel dropped error.
    #[error("Channel dropped error")]
    ChannelDropped,
    /// Not implemented error.
    #[error("Not implemented error")]
    NotImplemented,
    /// Missing provider name.
    #[error("Missing provider name")]
    MissingProviderName,
    /// Credentials error.
    #[error("Credentials error")]
    CredentialsError(#[from] CredentialsError),
}

/// Authentication error type for identity manager.
#[derive(Error, Debug)]
pub enum AuthenticationError {
    /// Signing key not found in credential.
    #[error("Signing key not found in credential.")]
    SigningKeyNotFound,
    /// Invalid private key length.
    #[error("Invalid private key length.")]
    InvalidPrivateKeyLength,
    /// Invalid public key length.
    #[error("Invalid public key length.")]
    InvalidPublicKeyLength,
    /// Invalid public key.
    #[error("Invalid public key.")]
    InvalidPublicKey,
    /// Invalid signature length.
    #[error("Invalid signature length.")]
    InvalidSignatureLength,
    /// Credentials error.
    #[error("Credentials error")]
    CredentialsError(#[from] CredentialsError),
    /// Encrypted identity feature is not enabled.
    #[error("Encrypted identity feature is not enabled.")]
    EncryptedIdentityFeatureNotEnabled,
}

/// Credential management error type for identity manager.
#[derive(Error, Debug)]
pub enum CredentialsError {
    /// Provider is disabled in cross-device settings.
    #[error("The requested identity provider is disabled in cross-device settings.")]
    CrossDeviceSettingsDisabledForProvider,
    /// Invalid broadcast credentials.
    #[error("Invalid broadcast credentials.")]
    InvalidBroadcastCredentials,
    /// Invalid visibility scope.
    #[error("Invalid visibility scope")]
    InvalidVisibilityScope,
    /// Credential not found.
    #[error("Credential not found.")]
    CredentialNotFound,
    /// Invalid key material.
    #[error("Invalid key material")]
    InvalidCredentialMaterial,
    ///Storage error.
    #[error("Storage error")]
    StorageError(#[from] StorageError),
    /// Channel sending error.
    #[error("Peer credentials changed transmission error")]
    SendError(#[from] tokio::sync::watch::error::SendError<()>),
    /// Failed to update V0 consumed salts.
    #[error("Failed to update V0 consumed salts")]
    V0ConsumedSaltsFailure,
}
