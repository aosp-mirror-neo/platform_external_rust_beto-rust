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

use super::{CredentialsDatabase, CREDENTIALS_FILE_PATH};
use credential_proto::proto::credential_book::{
    ConsumedSaltsBook, LocalCredentialBook, RemoteCredentialBook,
};
use identity_common::definitions::ProviderName;
use protobuf::Message as _;
use storage::{DatabaseStorage, StorageError};

const SELF_CREDENTIALS_PREFIX: &str = "local_credentials/";
const PEER_CREDENTIALS_PREFIX: &str = "remote_credentials/";
const CONSUMED_SALTS_PREFIX: &str = "consumed_salts/";

pub(crate) struct CredentialsDatabaseImpl<S: DatabaseStorage> {
    database: S,
}

impl<S: DatabaseStorage> CredentialsDatabase for CredentialsDatabaseImpl<S> {
    fn insert_local_credentials(
        &self,
        provider_name: &ProviderName,
        local_credentials: LocalCredentialBook,
    ) -> Result<(), StorageError> {
        let root_key = format!("{}{}", SELF_CREDENTIALS_PREFIX, provider_name.0);
        let credentials_bytes =
            local_credentials.write_to_bytes().map_err(|_| StorageError::SerializationFailure)?;
        let _ = self.database.insert(root_key.as_str(), credentials_bytes);
        Ok(())
    }

    fn insert_remote_credentials(
        &self,
        provider_name: &ProviderName,
        remote_credentials: RemoteCredentialBook,
    ) -> Result<(), StorageError> {
        let root_key = format!("{}{}", PEER_CREDENTIALS_PREFIX, provider_name.0);
        let credentials_bytes =
            remote_credentials.write_to_bytes().map_err(|_| StorageError::SerializationFailure)?;
        let _ = self.database.insert(root_key.as_str(), credentials_bytes);
        Ok(())
    }

    fn insert_consumed_salts(
        &self,
        provider_name: &ProviderName,
        consumed_salts: ConsumedSaltsBook,
    ) -> Result<(), StorageError> {
        let root_key = format!("{}{}", CONSUMED_SALTS_PREFIX, provider_name.0);
        let consumed_salts_bytes =
            consumed_salts.write_to_bytes().map_err(|_| StorageError::SerializationFailure)?;
        let _ = self.database.insert(root_key.as_str(), consumed_salts_bytes);
        Ok(())
    }

    fn get_local_credentials(
        &self,
        provider_name: &ProviderName,
    ) -> Result<LocalCredentialBook, StorageError> {
        let root_key = format!("{}{}", SELF_CREDENTIALS_PREFIX, provider_name.0);
        if let Some(retrieved_bytes) = self.database.get(root_key.as_str())? {
            let local_credential_book = LocalCredentialBook::parse_from_bytes(&retrieved_bytes[..])
                .map_err(|_| StorageError::SerializationFailure)?;
            // println!("Successfully retrieved local credential book!");
            Ok(local_credential_book)
        } else {
            Err(StorageError::SerializationFailure)
        }
    }

    fn get_remote_credentials(
        &self,
        provider_name: &ProviderName,
    ) -> Result<RemoteCredentialBook, StorageError> {
        let root_key = format!("{}{}", PEER_CREDENTIALS_PREFIX, provider_name.0);
        if let Some(retrieved_bytes) = self.database.get(root_key.as_str())? {
            let remote_credential_book =
                RemoteCredentialBook::parse_from_bytes(&retrieved_bytes[..])
                    .map_err(|_| StorageError::SerializationFailure)?;
            // println!("Successfully retrieved local credential book!");
            Ok(remote_credential_book)
        } else {
            Err(StorageError::SerializationFailure)
        }
    }

    fn get_consumed_salts(
        &self,
        provider_name: &ProviderName,
    ) -> Result<ConsumedSaltsBook, StorageError> {
        let root_key = format!("{}{}", CONSUMED_SALTS_PREFIX, provider_name.0);
        if let Some(retrieved_bytes) = self.database.get(root_key.as_str())? {
            let consumed_salts_book = ConsumedSaltsBook::parse_from_bytes(&retrieved_bytes[..])
                .map_err(|_| StorageError::SerializationFailure)?;
            Ok(consumed_salts_book)
        } else {
            Err(StorageError::SerializationFailure)
        }
    }

    fn delete_all_credentials(&self) -> Result<(), StorageError> {
        for prefix in [SELF_CREDENTIALS_PREFIX, PEER_CREDENTIALS_PREFIX, CONSUMED_SALTS_PREFIX] {
            for (key, _) in self.database.get_by_prefix(prefix)? {
                let _ = self.database.remove(&key)?;
            }
        }
        Ok(())
    }

    fn delete_credentials_for_provider(
        &self,
        provider_name: &ProviderName,
    ) -> Result<(), StorageError> {
        let self_key = format!("{}{}", SELF_CREDENTIALS_PREFIX, provider_name.0);
        let _ = self.database.remove(&self_key)?;

        let peer_key = format!("{}{}", PEER_CREDENTIALS_PREFIX, provider_name.0);
        let _ = self.database.remove(&peer_key)?;

        let consumed_salts_key = format!("{}{}", CONSUMED_SALTS_PREFIX, provider_name.0);
        let _ = self.database.remove(&consumed_salts_key)?;

        Ok(())
    }
}

impl<S: DatabaseStorage> CredentialsDatabaseImpl<S> {
    pub(crate) fn new(database: S) -> CredentialsDatabaseImpl<S> {
        CredentialsDatabaseImpl { database }
    }
}
