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

use async_trait::async_trait;
use credential_proto::proto::credential::DiscoveryCredential;
use identity_common::definitions::VisibilityLevel;
#[cfg(feature = "test-utils")]
use mockall::automock;
use std::collections::HashMap;
use tokio::sync::mpsc;

/// Identity provider events that are sent to the manager.
#[derive(Debug, Clone)]
pub enum IdentityProviderEvent {
    /// Remote credentials synced.
    RemoteCredentialsSynced {
        /// Synced remote discovery credentials.
        remote_credentials: Vec<DiscoveryCredential>,
    },
}

/// Identity provider errors.
#[derive(Debug, Clone)]
pub enum IdentityProviderError {
    /// Failed to sign data.
    SignFailure,
}

/// Identity Provider trait.
#[cfg_attr(feature = "test-utils", automock)]
#[async_trait]
pub trait IdentityProvider: Send + Sync {
    /// Provider registration succeeded.
    /// The manager passes back a provider id and an identity provider event sender channel.
    async fn on_provider_registered(&mut self, event_sender: mpsc::Sender<IdentityProviderEvent>);

    /// Signs the given data using the broadcast credential with the given id.
    fn sign(
        &self,
        broadcast_credential_id: i32,
        data: &[u8],
    ) -> Result<Vec<u8>, IdentityProviderError>;

    /// Request the identity provider update its local discovery credentials.
    /// The provider must:
    /// 1. Mint signing keys for the new local discovery credentials. Unchanged credentials do not need new signing keys.
    /// 2. Overwrite the existing set of local discovery credentials with the new set.
    /// 3. Distribute the updated set of local discovery credentials via an out-of-band channel (e.g. a server).
    /// 4. Return from the function once the local discovery credentials have been updated with signing keys and distributed.
    async fn update_local_discovery_credentials(
        &self,
        local_discovery_credentials: HashMap<VisibilityLevel, Vec<DiscoveryCredential>>,
    );
}
