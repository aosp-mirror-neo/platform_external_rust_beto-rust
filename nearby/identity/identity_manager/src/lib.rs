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

//! Cross-device identity manager. Supports:
//! 1. Registration of external identity provider.
//! 2. Management of common identity pool for D2DI features.
//! 3. Entry point for authentication functionality in Nearby.

/// Cross-device Identity Manager API for a multi-provider identity model.
pub mod api;
/// Error types for the identity manager crate.
pub mod errors;
/// Data types consumed only by the Nearby core and Codec crates.
pub mod types;

// Internal crate modules:

/// Implementation of Auth trait.
#[cfg(feature = "encrypted_identity")]
pub(crate) mod auth;
/// Credential management module.
#[cfg(feature = "encrypted_identity")]
pub mod credentials;
/// Storage implementations for Identity Manager.
pub mod identity_storage;
/// Implementation of Identity Manager with support for encrypted identity.
#[cfg(feature = "encrypted_identity")]
pub(crate) mod manager_impl;
/// Implementation of a public-only Identity Manager.
#[cfg(not(feature = "encrypted_identity"))]
pub(crate) mod public_manager_impl;

use crate::errors::IdentityManagerError;
use crypto_provider::CryptoProvider;

/// Retrieves an instance of the store that supports a key-value abstraction.
#[cfg(feature = "encrypted_identity")]
pub fn get_identity_manager_with_default_storage<C>(
) -> Result<impl api::IdentityManager + api::Authenticator, IdentityManagerError>
where
    C: CryptoProvider + Sync + 'static,
{
    manager_impl::IdentityManagerImpl::<C>::new()
}

/// Retrieves an instance of the identity manager used for verifying and generating
/// credentials, with injected storage dependencies.
#[cfg(feature = "encrypted_identity")]
pub fn get_identity_manager_with_storage<C, S, D>(
    preferences_storage: S,
    credentials_storage: D,
) -> Result<impl api::IdentityManager + api::Authenticator, IdentityManagerError>
where
    C: CryptoProvider + Sync + 'static,
    S: storage::PreferenceKeyValueStorage + 'static,
    D: storage::DatabaseStorage + 'static,
{
    manager_impl::IdentityManagerImpl::<C>::new_with_storage(
        preferences_storage,
        credentials_storage,
    )
}

/// Retrieves an instance of the identity manager with a store that supports a key-value abstraction.
#[cfg(not(feature = "encrypted_identity"))]
pub fn get_identity_manager_with_default_storage<C>(
) -> Result<impl api::IdentityManager, IdentityManagerError>
where
    C: CryptoProvider + Sync + 'static,
{
    public_manager_impl::IdentityManagerImpl::<C>::new()
}

/// Retrieves an instance of the identity manager with a given store that supports a key-value abstraction.
#[cfg(not(feature = "encrypted_identity"))]
pub fn get_identity_manager_with_storage<C, S>(
    preferences_storage: S,
) -> Result<impl api::IdentityManager, IdentityManagerError>
where
    C: CryptoProvider + Sync + 'static,
    S: storage::PreferenceKeyValueStorage + 'static,
{
    public_manager_impl::IdentityManagerImpl::<C>::new_with_storage(preferences_storage)
}

#[cfg(test)]
mod tests;
