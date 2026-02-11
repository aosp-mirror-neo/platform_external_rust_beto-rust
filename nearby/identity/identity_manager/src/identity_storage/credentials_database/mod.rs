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

use std::{fs, sync::Arc};

use credential_proto::proto::broadcast_credential::BroadcastCredential;
use credential_proto::proto::credential::DiscoveryCredential;
use credential_proto::proto::credential_book::{
    ConsumedSaltsBook, LocalCredentialBook, RemoteCredentialBook,
};
use identity_common::definitions::ProviderName;
use storage::{DatabaseStorage, PlatformDatabaseImpl, StorageError};

pub(crate) mod credentials_database_impl;

pub(crate) const CREDENTIALS_FILE_PATH: &str = "Google/Nearby/identity/credentials.json";

/// Encapsulates all the broadcast and discovery credentials for the local device.
#[derive(Clone)]
pub(crate) struct LocalCredentials {
    broadcast_credentials: Vec<BroadcastCredential>,
    discovery_credentials: Vec<DiscoveryCredential>,
}

/// Encapsulates all the broadcast and discovery credentials for the remote peer devices.
#[derive(Clone)]
pub(crate) struct RemoteCredentials {
    private_remote_credentials: Vec<DiscoveryCredential>,
    contacts_remote_credentials: Vec<DiscoveryCredential>,
}

pub(crate) trait CredentialsDatabase: Send + Sync {
    fn insert_local_credentials(
        &self,
        provider_name: &ProviderName,
        local_credentials: LocalCredentialBook,
    ) -> Result<(), StorageError>;

    fn insert_remote_credentials(
        &self,
        provider_name: &ProviderName,
        remote_credentials: RemoteCredentialBook,
    ) -> Result<(), StorageError>;

    fn insert_consumed_salts(
        &self,
        provider_name: &ProviderName,
        consumed_salts: ConsumedSaltsBook,
    ) -> Result<(), StorageError>;

    fn get_local_credentials(
        &self,
        provider_name: &ProviderName,
    ) -> Result<LocalCredentialBook, StorageError>;

    fn get_remote_credentials(
        &self,
        provider_name: &ProviderName,
    ) -> Result<RemoteCredentialBook, StorageError>;

    fn get_consumed_salts(
        &self,
        provider_name: &ProviderName,
    ) -> Result<ConsumedSaltsBook, StorageError>;

    fn delete_all_credentials(&self) -> Result<(), StorageError>;

    fn delete_credentials_for_provider(
        &self,
        provider_name: &ProviderName,
    ) -> Result<(), StorageError>;
}

pub(crate) fn create_database_storage() -> Result<impl DatabaseStorage, StorageError> {
    let local_app_data_dir = storage::get_local_app_data_dir()?;
    let full_path = local_app_data_dir.join(CREDENTIALS_FILE_PATH);
    // SAFETY: parent folder always exists.
    #[allow(clippy::expect_used)]
    fs::create_dir_all(full_path.parent().expect("No parent directory"))
        .expect("Failed to create credentials storage");
    let database = PlatformDatabaseImpl::new(full_path)?;
    Ok(database)
}

pub(crate) fn create_credentials_database(
    database: impl DatabaseStorage + 'static,
) -> Arc<dyn CredentialsDatabase> {
    log::info!("Credentials storage created");
    Arc::new(credentials_database_impl::CredentialsDatabaseImpl::new(database))
}
