// Copyright 2026 Google LLC
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

use crate::credentials::EncryptedMetadata;
use crate::identity_storage::credentials_database::CredentialsDatabase;
use credential_proto::proto::credential::DiscoveryCredential;
use identity_common::definitions::ProviderName;
use std::sync::Arc;
use up_adv::credential::book::CredentialBook;
use up_adv::credential::matched::MatchedCredentialWithId;
use up_adv::credential::v0::V0DiscoveryCredential;
use up_adv::credential::v1::V1DiscoveryCredential;

/// A credential book that loads credentials from the database on demand.
///
/// This avoids loading all credentials into memory when they effectively might not be needed.
pub struct LazyDiscoveryCredentialBook {
    database: Arc<dyn CredentialsDatabase>,
    provider_names: Vec<ProviderName>,
}

impl LazyDiscoveryCredentialBook {
    /// Creates a new `LazyDiscoveryCredentialBook`.
    pub(crate) fn new(
        database: Arc<dyn CredentialsDatabase>,
        provider_names: Vec<ProviderName>,
    ) -> Self {
        Self { database, provider_names }
    }

    fn iter_creds<'a, T, F>(
        database: &'a dyn CredentialsDatabase,
        provider_names: &'a [ProviderName],
        map_fn: F,
    ) -> Box<dyn Iterator<Item = (T, EncryptedMetadata)> + 'a + Send>
    where
        F: Fn(DiscoveryCredential) -> Option<(T, EncryptedMetadata)> + 'a + Send + Clone,
    {
        Box::new(
            provider_names
                .iter()
                .flat_map(move |provider_name| {
                    database
                        .get_remote_credentials(provider_name)
                        .ok()
                        .map(|book| book.contacts_remote_credentials)
                        .unwrap_or_default()
                })
                .filter_map(map_fn),
        )
    }
}

impl<'a> CredentialBook<'a> for LazyDiscoveryCredentialBook {
    type Matched = EncryptedMetadata;
    type V0Crypto = V0DiscoveryCredential;
    type V1Crypto = V1DiscoveryCredential;
    type V0Iterator = Box<dyn Iterator<Item = (Self::V0Crypto, Self::Matched)> + 'a + Send>;
    type V1Iterator = Box<dyn Iterator<Item = (Self::V1Crypto, Self::Matched)> + 'a + Send>;

    fn v0_iter(&'a self) -> Self::V0Iterator {
        Self::iter_creds(self.database.as_ref(), &self.provider_names, |cred| {
            let key_seed: [u8; 32] = cred.key_seed.clone().try_into().ok()?;
            let hmac: [u8; 32] = cred.identity_token_hmac_key_v0.clone().try_into().ok()?;
            let v0_cred = V0DiscoveryCredential::new(key_seed, hmac);
            Some((v0_cred, EncryptedMetadata::new(cred.encrypted_metadata_bytes_v0, cred.id)))
        })
    }

    fn v1_iter(&'a self) -> Self::V1Iterator {
        Self::iter_creds(self.database.as_ref(), &self.provider_names, |cred| {
            let key_seed: [u8; 32] = cred.key_seed.clone().try_into().ok()?;
            let short_hmac: [u8; 32] =
                cred.identity_token_short_salt_adv_hmac_key_v1.clone().try_into().ok()?;
            let extended_hmac: [u8; 32] =
                cred.identity_token_extended_salt_adv_hmac_key_v1.clone().try_into().ok()?;
            let v1_cred = V1DiscoveryCredential::new(key_seed, short_hmac, extended_hmac);
            Some((v1_cred, EncryptedMetadata::new(cred.encrypted_metadata_bytes_v1, cred.id)))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity_storage::credentials_database::CredentialsDatabase;
    use credential_proto::proto::credential_book::RemoteCredentialBook;
    use std::sync::Mutex;
    use storage::StorageError;

    struct MockDatabase {
        calls: Mutex<Vec<ProviderName>>,
    }

    impl CredentialsDatabase for MockDatabase {
        fn get_remote_credentials(
            &self,
            provider_name: &ProviderName,
        ) -> Result<RemoteCredentialBook, StorageError> {
            #[allow(clippy::unwrap_used)]
            self.calls.lock().unwrap().push(provider_name.clone());
            Ok(RemoteCredentialBook::default())
        }
        fn insert_remote_credentials(
            &self,
            _provider_name: &ProviderName,
            _remote_credentials: RemoteCredentialBook,
        ) -> Result<(), StorageError> {
            Ok(())
        }

        fn insert_local_credentials(
            &self,
            _: &ProviderName,
            _: credential_proto::proto::credential_book::LocalCredentialBook,
        ) -> Result<(), StorageError> {
            Ok(())
        }
        fn get_local_credentials(
            &self,
            _: &ProviderName,
        ) -> Result<credential_proto::proto::credential_book::LocalCredentialBook, StorageError>
        {
            Ok(Default::default())
        }
        fn insert_consumed_salts(
            &self,
            _: &ProviderName,
            _: credential_proto::proto::credential_book::ConsumedSaltsBook,
        ) -> Result<(), StorageError> {
            Ok(())
        }
        fn get_consumed_salts(
            &self,
            _: &ProviderName,
        ) -> Result<credential_proto::proto::credential_book::ConsumedSaltsBook, StorageError>
        {
            Ok(Default::default())
        }
        fn delete_all_credentials(&self) -> Result<(), StorageError> {
            Ok(())
        }
        fn delete_credentials_for_provider(&self, _: &ProviderName) -> Result<(), StorageError> {
            Ok(())
        }
    }

    #[allow(clippy::unwrap_used)]
    #[test]
    fn test_lazy_loading() {
        let db = Arc::new(MockDatabase { calls: Mutex::new(Vec::new()) });
        let provider_names = vec![ProviderName("p1".into()), ProviderName("p2".into())];
        let book = LazyDiscoveryCredentialBook::new(db.clone(), provider_names.clone());

        // Initially no calls
        assert!(db.calls.lock().unwrap().is_empty());

        // Iterate
        let mut v1_iter = book.v1_iter();

        // Next triggers load of p1
        assert!(v1_iter.next().is_none()); // Mock returns empty book

        let calls = db.calls.lock().unwrap();
        // It might load p1 then p2 looking for credentials?
        // Since both return empty, it will exhaust both.
        assert_eq!(calls.len(), 2);
        assert_eq!(calls.first().unwrap().0, "p1");
        assert_eq!(calls.last().unwrap().0, "p2");
    }
}
