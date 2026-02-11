#![allow(clippy::unwrap_used)]
use credential_proto::proto::credential_book::ConsumedSaltsBook;
use identity_common::definitions::ProviderName;
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
use crate::identity_storage::credentials_database::CredentialsDatabase;
use credential_proto::proto::credential_book::{LocalCredentialBook, RemoteCredentialBook};
use std::collections::HashMap;
use std::sync::Mutex;
use storage::StorageError;
pub(crate) struct FakeCredentialsStorage {
    local_creds: Mutex<HashMap<ProviderName, LocalCredentialBook>>,
    remote_creds: Mutex<HashMap<ProviderName, RemoteCredentialBook>>,
    consumed_salts: Mutex<HashMap<ProviderName, ConsumedSaltsBook>>,
}

impl FakeCredentialsStorage {
    pub(crate) fn new() -> Self {
        Self {
            local_creds: Mutex::new(HashMap::new()),
            remote_creds: Mutex::new(HashMap::new()),
            consumed_salts: Mutex::new(HashMap::new()),
        }
    }
}

impl CredentialsDatabase for FakeCredentialsStorage {
    fn get_local_credentials(
        &self,
        provider_name: &ProviderName,
    ) -> Result<LocalCredentialBook, StorageError> {
        self.local_creds.lock().unwrap().get(provider_name).cloned().ok_or(
            StorageError::FileBasedStorageError("credential not found in fake storage".to_string()),
        )
    }

    fn insert_local_credentials(
        &self,
        provider_name: &ProviderName,
        book: LocalCredentialBook,
    ) -> Result<(), StorageError> {
        let _ = self.local_creds.lock().unwrap().insert(provider_name.clone(), book);
        Ok(())
    }

    fn get_remote_credentials(
        &self,
        provider_name: &ProviderName,
    ) -> Result<RemoteCredentialBook, StorageError> {
        self.remote_creds.lock().unwrap().get(provider_name).cloned().ok_or(
            StorageError::FileBasedStorageError("credential not found in fake storage".to_string()),
        )
    }

    fn insert_remote_credentials(
        &self,
        provider_name: &ProviderName,
        book: RemoteCredentialBook,
    ) -> Result<(), StorageError> {
        let _ = self.remote_creds.lock().unwrap().insert(provider_name.clone(), book);
        Ok(())
    }

    fn insert_consumed_salts(
        &self,
        provider_name: &ProviderName,
        consumed_salts: ConsumedSaltsBook,
    ) -> Result<(), StorageError> {
        let _ = self.consumed_salts.lock().unwrap().insert(provider_name.clone(), consumed_salts);
        Ok(())
    }

    fn get_consumed_salts(
        &self,
        provider_name: &ProviderName,
    ) -> Result<ConsumedSaltsBook, StorageError> {
        self.consumed_salts.lock().unwrap().get(provider_name).cloned().ok_or(
            StorageError::FileBasedStorageError(
                "consumed salts not found in fake storage".to_string(),
            ),
        )
    }

    fn delete_credentials_for_provider(
        &self,
        provider_name: &ProviderName,
    ) -> Result<(), StorageError> {
        let _ = self.local_creds.lock().unwrap().remove(provider_name);
        let _ = self.remote_creds.lock().unwrap().remove(provider_name);
        let _ = self.consumed_salts.lock().unwrap().remove(provider_name);
        Ok(())
    }

    fn delete_all_credentials(&self) -> Result<(), StorageError> {
        self.local_creds.lock().unwrap().clear();
        self.remote_creds.lock().unwrap().clear();
        self.consumed_salts.lock().unwrap().clear();
        Ok(())
    }
}
