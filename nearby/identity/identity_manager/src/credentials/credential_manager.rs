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

use crate::{credentials::EncryptedMetadata, errors::CredentialsError, types::AuthToken};
use credential_proto::proto::{
    broadcast_credential::BroadcastCredential, credential::DiscoveryCredential,
    credential_book::LocalCredentialBook,
};
use identity_common::definitions::{DeviceInfo, ProviderName, VisibilityLevel, VisibilityScope};
use ldt_np_adv::{V0IdentityToken, V0Salt};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, watch};
use up_adv::credential::{
    book::PrecalculatedOwnedCredentialBookWithId, v0::V0BroadcastCredential,
    v1::V1BroadcastCredential,
};

/// A trait to manage the lifecycle of credentials, store and cache credential materials,
/// and fetch credentials in a prepared state for encoding, decoding, and authentication.
pub(crate) trait CredentialManager: Send + Sync {
    /// Fetch a V0 Broadcast Credential prepared for `up_adv` V0 serialization.
    fn get_v0_broadcast_credential_with_salt(
        &self,
        visibility: &VisibilityLevel,
    ) -> Result<(V0BroadcastCredential, V0Salt), CredentialsError>;

    /// Fetch a V1 broadcast credential prepared for `up_adv` V1 serialization.
    fn get_v1_broadcast_credential(
        &self,
        visibility: &VisibilityScope,
    ) -> Result<V1BroadcastCredential, CredentialsError>;

    /// Fetch the private group discovery credential book (includes self + family scope credentials).
    fn get_private_group_discovery_credential_book(
        &self,
    ) -> Result<Arc<PrecalculatedOwnedCredentialBookWithId<EncryptedMetadata>>, CredentialsError>;

    /// Fetch a frequently used contact discovery credential book (LRU cache).
    fn get_frequent_contact_discovery_credential_book(
        &self,
    ) -> Result<Arc<PrecalculatedOwnedCredentialBookWithId<EncryptedMetadata>>, CredentialsError>;

    /// Fetch an on-demand contact discovery credential book prepared for `up_adv` deserialization.
    fn get_on_demand_contact_discovery_credential_book(
        &self,
    ) -> Result<crate::credentials::lazy_book::LazyDiscoveryCredentialBook, CredentialsError>;

    /// Notify that a contact credential with the given ID successfully decrypted an advertisement.
    fn notify_contact_credential_used(&self, id: i64);

    /// Fetch a broadcast credential id to call `IdentityProvider#sign` with.
    fn get_signing_credential_id(&self, token: &AuthToken) -> Result<i64, CredentialsError>;

    /// Fetch a discovery credential with ed25519 public key material for signature verification.
    fn get_verifying_discovery_credential(
        &self,
        token: &AuthToken,
    ) -> Result<DiscoveryCredential, CredentialsError>;

    /// Update the remote downloaded remote credentials for the associated identity provider.
    /// This is called during a remote credentials sync event from a registered identity provider.
    fn update_remote_credentials(
        &self,
        provider_name: &ProviderName,
        credentials: Vec<DiscoveryCredential>,
    ) -> Result<(), CredentialsError>;

    //// Wipeout all credentials for the associated identity provider + optional visibility level.
    fn wipeout_credentials(
        &self,
        provider_name: &ProviderName,
        visibility_level: Option<VisibilityLevel>,
    ) -> Result<(), CredentialsError>;

    /// Replenish a full set of credentials for the associated identity provider + optional visibility level.
    fn replenish_credentials(
        &self,
        provider_name: &ProviderName,
        visibility_level: Option<VisibilityLevel>,
    ) -> Result<(), CredentialsError>;

    /// Update storage and cache with the local credential book which has been "primed" with signing keys by the respective identity provider.
    /// These credentials will now be used for advertisement encryption and connection signing.
    fn save_primed_credentials(
        &self,
        provider_name: &ProviderName,
        local_credential_book: LocalCredentialBook,
    ) -> Result<(), CredentialsError>;

    /// Subscribe to broadcast credential changes.
    fn subscribe_broadcast_credential_changes(&self) -> broadcast::Receiver<VisibilityScope>;

    /// Subscribe to local discovery credential changes.
    fn subscribe_local_discovery_credential_changes(
        &self,
    ) -> broadcast::Receiver<LocalDiscoveryCredentials>;

    /// Subscribe to remote discovery credential changes.
    fn subscribe_remote_discovery_credential_changes(&self) -> watch::Receiver<()>;

    /// Derive visibility scope of a decrypted advertisement from the decrypted discovery credential ID.
    fn get_visibility_scope_from_id(&self, id: i64) -> Option<VisibilityScope>;
}

/// Local discovery credentials.
#[derive(Debug, Clone)]
pub(crate) struct LocalDiscoveryCredentials(pub(crate) (ProviderName, LocalCredentialBook));
