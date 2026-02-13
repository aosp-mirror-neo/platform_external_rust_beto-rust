// Copyright 2025 Google LLC
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

use crate::api::{IdentityProvider, IdentityProviderError, IdentityProviderEvent};
use async_trait::async_trait;
use credential_proto::proto::credential::DiscoveryCredential;
use identity_common::definitions::VisibilityLevel;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// A fake implementation of `IdentityProvider` for testing purposes.
#[derive(Default, Clone)]
pub struct FakeIdentityProvider {
    event_sender: Arc<Mutex<Option<mpsc::Sender<IdentityProviderEvent>>>>,
    local_credentials: Arc<Mutex<HashMap<VisibilityLevel, Vec<DiscoveryCredential>>>>,
}

impl FakeIdentityProvider {
    /// Creates a new `FakeIdentityProvider`.
    pub fn new() -> Self {
        Default::default()
    }

    /// Injects remote credentials into the identity manager by sending a `RemoteCredentialsSynced` event.
    #[allow(clippy::unwrap_used)]
    pub async fn inject_remote_credentials(&self, remote_credentials: Vec<DiscoveryCredential>) {
        let sender = self.event_sender.lock().unwrap().clone();
        if let Some(sender) = sender {
            let _ = sender
                .send(IdentityProviderEvent::RemoteCredentialsSynced { remote_credentials })
                .await;
        }
    }

    /// Retrieves the local credentials that were last sent to this provider.
    #[allow(clippy::unwrap_used)]
    pub fn get_local_credentials(&self) -> HashMap<VisibilityLevel, Vec<DiscoveryCredential>> {
        self.local_credentials.lock().unwrap().clone()
    }
}

#[async_trait]
impl IdentityProvider for FakeIdentityProvider {
    #[allow(clippy::unwrap_used)]
    async fn on_provider_registered(&mut self, event_sender: mpsc::Sender<IdentityProviderEvent>) {
        *self.event_sender.lock().unwrap() = Some(event_sender);
    }

    fn sign(
        &self,
        _broadcast_credential_id: i32,
        _data: &[u8],
    ) -> Result<Vec<u8>, IdentityProviderError> {
        unimplemented!()
    }

    #[allow(clippy::unwrap_used)]
    async fn update_local_discovery_credentials(
        &self,
        local_discovery_credentials: HashMap<VisibilityLevel, Vec<DiscoveryCredential>>,
    ) {
        *self.local_credentials.lock().unwrap() = local_discovery_credentials;
    }
}
