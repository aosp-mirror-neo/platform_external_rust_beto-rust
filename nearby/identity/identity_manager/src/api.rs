// Copyright 2023 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
use crate::{
    errors::{AuthenticationError, IdentityManagerError},
    types::{BroadcastSessionId, StableDeviceId},
};

use async_trait::async_trait;
use identity_common::definitions::{DeviceInfo, DeviceType, VisibilityScope};
#[cfg(feature = "test-utils")]
use mockall::automock;

/// Identity management trait for registering external identity providers and syncing multidevice settings.
#[cfg_attr(feature = "test-utils", automock)]
#[async_trait]
pub trait IdentityManager: Send + Sync {
    /// Register an identity provider with an identity provider configuration.
    /// The identity provider will be automatically unregistered when the underlying implementation
    /// drops the IdentityProviderEvent sender channel received on registration.
    #[cfg(feature = "encrypted_identity")]
    fn register_identity_provider(
        &self,
        provider: Box<dyn identity_provider::api::IdentityProvider>,
        config: identity_common::definitions::IdentityProviderConfig,
    );

    /// An identity provider requests for the system to regenerate credentials for
    /// a given visibility scope.
    /// -> SELF scope rotate: this will rotate the stable device ID and regenerate SELF
    /// scope credentials for *all* identity providers.
    /// -> CONTACT scope rotate: this will rotate CONTACT scope credentials *only* for the
    /// identity provider that requested the rotation.
    #[cfg(feature = "encrypted_identity")]
    async fn rotate_credentials(
        &self,
        provider_name: identity_common::definitions::ProviderName,
        visibility_level: identity_common::definitions::VisibilityLevel,
    ) -> Result<(), IdentityManagerError>;

    /// Update the device information for metadata generation.
    async fn update_device_info(&self, device_info: DeviceInfo)
        -> Result<(), IdentityManagerError>;

    /// The name of the local device.
    ///
    /// This name is used to identify the device to other devices during connection.
    fn local_device_name(&self) -> String;

    /// The type of the local device.
    ///
    /// This helps the remote device to display the correct icon for this device.
    fn local_device_type(&self) -> DeviceType;

    /// The stable device ID of the local device.
    ///
    /// This enables the Codec to dedupe public and non-public devices.
    fn local_device_id(&self) -> StableDeviceId;

    /// Rotates the broadcast session ID used in public advertisements.
    fn rotate_broadcast_session_id(&self);

    /// The broadcast session ID of the local device.
    ///
    /// This is used to prevent connecting to the same device multiple times.
    fn local_broadcast_session_id(&self) -> BroadcastSessionId;

    /// Encrypted local identity, which can be decoded if the peer owns our discovery credential.
    ///
    /// This is used to prove our identity to the remote device.
    fn encrypted_local_identity(&self) -> Option<Vec<u8>>;

    /// Fetch a V0 Broadcast Credential prepared for `up_adv` V0 serialization.
    #[cfg(feature = "encrypted_identity")]
    fn get_v0_broadcast_credential_with_salt(
        &self,
        visibility: &identity_common::definitions::VisibilityLevel,
    ) -> Result<
        (up_adv::credential::v0::V0BroadcastCredential, ldt_np_adv::V0Salt),
        crate::errors::CredentialsError,
    >;

    /// Fetch a V1 broadcast credential prepared for `up_adv` V1 serialization.
    #[cfg(feature = "encrypted_identity")]
    fn get_v1_broadcast_credential(
        &self,
        visibility: &identity_common::definitions::VisibilityScope,
    ) -> Result<up_adv::credential::v1::V1BroadcastCredential, crate::errors::CredentialsError>;

    /// Fetch the private group discovery credential book (includes self + family scope credentials).
    #[cfg(feature = "encrypted_identity")]
    fn get_private_group_discovery_credential_book(
        &self,
    ) -> Result<
        std::sync::Arc<
            up_adv::credential::book::PrecalculatedOwnedCredentialBookWithId<
                crate::credentials::EncryptedMetadata,
            >,
        >,
        crate::errors::CredentialsError,
    >;

    /// Fetch a frequently used contact discovery credential book (LRU cache).
    #[cfg(feature = "encrypted_identity")]
    fn get_frequent_contact_discovery_credential_book(
        &self,
    ) -> Result<
        std::sync::Arc<
            up_adv::credential::book::PrecalculatedOwnedCredentialBookWithId<
                crate::credentials::EncryptedMetadata,
            >,
        >,
        crate::errors::CredentialsError,
    >;

    /// Fetch an on-demand contact discovery credential book prepared for `up_adv` deserialization.
    #[cfg(feature = "encrypted_identity")]
    fn get_on_demand_contact_discovery_credential_book(
        &self,
    ) -> Result<
        crate::credentials::lazy_book::LazyDiscoveryCredentialBook,
        crate::errors::CredentialsError,
    >;

    /// Notify that a contact credential with the given ID successfully decrypted an advertisement.
    #[cfg(feature = "encrypted_identity")]
    fn notify_contact_credential_used(&self, id: i64);

    /// Derive visibility scope of a decrypted advertisement from the decrypted discovery credential ID.
    #[cfg(feature = "encrypted_identity")]
    fn get_visibility_scope_from_id(
        &self,
        id: i64,
    ) -> Option<identity_common::definitions::VisibilityScope>;

    /// Broadcast credential change listener.
    async fn broadcast_credentials_changed(&self) -> Result<VisibilityScope, IdentityManagerError>;

    /// Peer discovery credential change listener.
    async fn discovery_credentials_changed(&self) -> Result<(), IdentityManagerError>;
}

/// Authenticator for use by connection authentication handshake layer in Nearby.
#[cfg_attr(feature = "test-utils", automock)]
pub trait Authenticator: Send + Sync {
    /// Sign {context + data} bytes with the broadcast credential hash auth token.
    fn sign(
        &self,
        token: &[u8],
        context: &[u8],
        data: &[u8],
    ) -> Result<Vec<u8>, AuthenticationError>;

    /// Verify incoming signature against {context + data} bytes with the discovery credential hash auth token.
    fn verify(
        &self,
        token: &[u8],
        context: &[u8],
        data: &[u8],
        signature: &[u8],
    ) -> Result<bool, AuthenticationError>;
}
