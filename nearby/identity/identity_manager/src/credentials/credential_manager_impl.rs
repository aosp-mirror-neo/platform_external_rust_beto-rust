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
    credentials::{
        credential_manager::{CredentialManager, LocalDiscoveryCredentials},
        utils::credentials,
        EncryptedMetadata,
    },
    errors::CredentialsError,
    identity_storage::{
        credentials_database::CredentialsDatabase, IdentityManagerPreferences, DEVICE_ID_LENGTH,
    },
    types::AuthToken,
};
use arc_swap::ArcSwap;
use chrono::{TimeZone, Utc};
use credential_proto::proto::broadcast_credential::BroadcastCredential;
use credential_proto::proto::credential::{
    DiscoveryCredential, VisibilityLevel as ProtoVisibilityLevel,
    VisibilityScope as ProtoVisibilityScope,
};
use credential_proto::proto::credential_book::{
    ConsumedSaltsBook, LocalCredentialBook, RemoteCredentialBook,
};
use credential_proto::proto::{
    self,
    credential::VisibilityLevel::{
        VISIBILITY_LEVEL_CONTACTS_GROUP, VISIBILITY_LEVEL_PUBLIC, VISIBILITY_LEVEL_SELF_GROUP,
        VISIBILITY_LEVEL_UNSPECIFIED,
    },
    metadata::{DeviceIdentityMetaData, DeviceType, InstanceType},
};
use crypto_provider::CryptoRng;
use crypto_provider::{ed25519::Ed25519Provider, sha2::Sha256, CryptoProvider};
use identity_common::definitions::{DeviceInfo, ProviderName, VisibilityLevel, VisibilityScope};
use ldt_np_adv::{V0IdentityToken, V0Salt, V0_IDENTITY_TOKEN_LEN, V0_SALT_LEN};
use log::info;
use protobuf::{Enum, Message as _};
use std::{
    cmp::max,
    collections::{HashMap, HashSet},
    marker::PhantomData,
    sync::Arc,
};
use storage::StorageError;
use tokio::{
    sync::{broadcast, mpsc, watch},
    task::JoinHandle,
};
use up_adv::{
    credential::{
        book::{CredentialBook, CredentialBookBuilder, PrecalculatedOwnedCredentialBookWithId},
        v0::{V0BroadcastCredential, V0DiscoveryCredential},
        v1::{V1BroadcastCredential, V1DiscoveryCredential},
        MatchableCredential,
    },
    extended::{V1IdentityToken, V1_IDENTITY_TOKEN_LEN},
};

pub(crate) const EXPECTED_NUM_LOCAL_CREDENTIALS: usize = 6;
const CREDENTIAL_ACTIVE_TIME_OFFSET_MILLIS: i64 = 30 * 60 * 1000;
/** Max number of salts to be used per certificate. */
const MAX_NUM_SALTS: usize = 32768;
/** Max number of retries to generate a salt. */
const MAX_NUM_RETRIES: usize = 256;
const MAX_CACHED_CONTACT_CREDENTIALS: usize = 100;

#[derive(Clone)]
struct CredentialsInner {
    broadcast_credentials: HashMap<ProviderName, HashMap<i32, Vec<BroadcastCredential>>>,
    // The book derived from `frequent_contact_credentials`.
    frequent_contact_discovery_credential_book:
        Arc<PrecalculatedOwnedCredentialBookWithId<EncryptedMetadata>>,
    // Cache (LRU eviction) of raw contact credentials used to build the frequent discovery credential book.
    frequent_contact_credentials: Vec<DiscoveryCredential>,
    // A precomputed book of all private group credentials (includes self + family scope credentials).
    private_group_discovery_credential_book:
        Arc<PrecalculatedOwnedCredentialBookWithId<EncryptedMetadata>>,
    unscoped_metadata: DeviceIdentityMetaData,
    provider_names: Vec<ProviderName>,
}

pub(crate) struct CredentialManagerImpl<C: CryptoProvider> {
    inner: Arc<ArcSwap<CredentialsInner>>,
    preferences: watch::Receiver<IdentityManagerPreferences>,
    credentials_storage: Arc<dyn CredentialsDatabase>,
    broadcast_credentials_changed: broadcast::Sender<VisibilityScope>,
    local_discovery_credentials_changed: broadcast::Sender<LocalDiscoveryCredentials>,
    remote_discovery_credentials_changed: watch::Sender<()>,
    preferences_update_handle: JoinHandle<()>,
    crypto_provider: PhantomData<C>,
}

impl<C> CredentialManagerImpl<C>
where
    C: CryptoProvider + Sync + Send,
    C::Ed25519: Ed25519Provider,
{
    pub(crate) fn new(
        preferences: watch::Receiver<IdentityManagerPreferences>,
        credentials_storage: Arc<dyn CredentialsDatabase>,
    ) -> Result<Self, StorageError> {
        let (broadcast_credentials_changed, _) = broadcast::channel(100);
        let (local_discovery_credentials_changed, _) = broadcast::channel(100);
        let (remote_discovery_credentials_changed, _) = watch::channel(());
        let cloned_preferences = preferences.clone();
        let initial_prefs = preferences.borrow().clone();
        let provider_names: Vec<ProviderName> =
            initial_prefs.provider_visibilities.keys().cloned().map(ProviderName).collect();
        let private_group_credentials =
            Self::load_all_private_group_credentials(&provider_names, credentials_storage.as_ref());
        let initial_inner =
            CredentialsInner {
                unscoped_metadata: Self::extract_unscoped_metadata(initial_prefs),
                private_group_discovery_credential_book:
                    Self::build_book_from_discovery_credentials(&private_group_credentials),
                frequent_contact_discovery_credential_book:
                    Self::build_book_from_discovery_credentials(&[]),
                provider_names,
                broadcast_credentials: HashMap::new(),
                frequent_contact_credentials: Vec::new(),
            };
        let inner: Arc<ArcSwap<CredentialsInner>> = Arc::new(ArcSwap::new(initial_inner.into()));
        let cloned_inner = inner.clone();
        let cloned_storage = credentials_storage.clone();
        let cloned_local_creds_sender = local_discovery_credentials_changed.clone();
        let preferences_update_handle = tokio::spawn(async move {
            Self::process_preferences_updates(
                cloned_preferences,
                cloned_inner,
                cloned_storage,
                cloned_local_creds_sender,
            )
            .await;
        });
        Ok(Self {
            inner,
            preferences,
            credentials_storage,
            broadcast_credentials_changed,
            local_discovery_credentials_changed,
            remote_discovery_credentials_changed,
            preferences_update_handle,
            crypto_provider: PhantomData,
        })
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(
        preferences: watch::Receiver<IdentityManagerPreferences>,
        credentials_storage: Arc<dyn CredentialsDatabase>,
        broadcast_credentials_changed: broadcast::Sender<VisibilityScope>,
        local_discovery_credentials_changed: broadcast::Sender<LocalDiscoveryCredentials>,
        remote_discovery_credentials_changed: watch::Sender<()>,
    ) -> Self {
        let cloned_preferences = preferences.clone();
        let initial_prefs = preferences.borrow().clone();
        let provider_names: Vec<ProviderName> =
            initial_prefs.provider_visibilities.keys().cloned().map(ProviderName).collect();
        let private_group_credentials =
            Self::load_all_private_group_credentials(&provider_names, credentials_storage.as_ref());
        let initial_inner =
            CredentialsInner {
                unscoped_metadata: Self::extract_unscoped_metadata(initial_prefs),
                private_group_discovery_credential_book:
                    Self::build_book_from_discovery_credentials(&private_group_credentials),
                frequent_contact_discovery_credential_book:
                    Self::build_book_from_discovery_credentials(&[]),
                provider_names,
                broadcast_credentials: HashMap::new(),
                frequent_contact_credentials: Vec::new(),
            };
        let inner: Arc<ArcSwap<CredentialsInner>> = Arc::new(ArcSwap::new(initial_inner.into()));
        let cloned_inner = inner.clone();
        let cloned_storage = credentials_storage.clone();
        let cloned_local_creds_sender = local_discovery_credentials_changed.clone();
        let preferences_update_handle = tokio::spawn(async move {
            Self::process_preferences_updates(
                cloned_preferences,
                cloned_inner,
                cloned_storage,
                cloned_local_creds_sender,
            )
            .await;
        });
        Self {
            inner,
            preferences,
            credentials_storage,
            broadcast_credentials_changed,
            local_discovery_credentials_changed,
            remote_discovery_credentials_changed,
            preferences_update_handle,
            crypto_provider: PhantomData,
        }
    }

    async fn process_preferences_updates(
        mut preferences: watch::Receiver<IdentityManagerPreferences>,
        inner: Arc<ArcSwap<CredentialsInner>>,
        credentials_storage: Arc<dyn CredentialsDatabase>,
        local_discovery_credentials_changed: broadcast::Sender<LocalDiscoveryCredentials>,
    ) {
        while preferences.changed().await.is_ok() {
            let prefs = preferences.borrow_and_update().clone();
            let new_provider_names: Vec<ProviderName> =
                prefs.provider_visibilities.keys().cloned().map(ProviderName).collect();
            let new_unscoped_metadata = Self::extract_unscoped_metadata(prefs);

            if inner.load().provider_names != new_provider_names {
                let _ = inner.rcu(|old_inner| {
                    let mut new_inner = old_inner.as_ref().clone();
                    new_inner.provider_names = new_provider_names.clone();
                    new_inner
                });
            }

            if new_unscoped_metadata != inner.load().unscoped_metadata {
                if let Err(e) = Self::refresh_encrypted_metadata_for_all_credentials(
                    new_unscoped_metadata,
                    preferences.clone(),
                    credentials_storage.clone(),
                    inner.clone(),
                    local_discovery_credentials_changed.clone(),
                )
                .await
                {
                    log::error!("Failed to refresh encrypted metadata: {e:?}");
                }
            }
        }
    }

    // TODO: add family remote credentials.
    fn load_all_private_group_credentials(
        provider_names: &[ProviderName],
        credentials_storage: &dyn CredentialsDatabase,
    ) -> Vec<DiscoveryCredential> {
        provider_names
            .iter()
            .flat_map(|provider_name| {
                credentials_storage
                    .get_remote_credentials(provider_name)
                    .ok()
                    .map(|book| book.self_remote_credentials)
                    .unwrap_or_default()
            })
            .collect()
    }

    async fn refresh_encrypted_metadata_for_all_credentials(
        new_metadata: DeviceIdentityMetaData,
        preferences: watch::Receiver<IdentityManagerPreferences>,
        credentials_storage: Arc<dyn CredentialsDatabase>,
        inner: Arc<ArcSwap<CredentialsInner>>,
        local_discovery_credentials_changed: broadcast::Sender<LocalDiscoveryCredentials>,
    ) -> Result<(), CredentialsError> {
        // First, update the in-memory cache.
        let _ = inner.rcu(|old_inner| {
            let mut new_inner = old_inner.as_ref().clone();
            new_inner.unscoped_metadata = new_metadata.clone();
            new_inner
        });

        for provider_name in inner.load().provider_names.iter() {
            let Ok(mut local_credential_book) =
                credentials_storage.get_local_credentials(provider_name)
            else {
                // If there are no credentials for this provider, just skip it.
                continue;
            };

            // Go through all scopes (self, contact, etc.) for the provider.
            local_credential_book.local_credentials.values_mut().for_each(|scoped_creds| {
                let broadcast_creds_map: HashMap<i64, &BroadcastCredential> =
                    scoped_creds.broadcast_credentials.iter().map(|c| (c.id, c)).collect();

                let updated_discovery_credentials: Vec<DiscoveryCredential> = scoped_creds
                    .converted_discovery_credentials
                    .iter()
                    .filter_map(|discovery_cred| {
                        broadcast_creds_map.get(&discovery_cred.id).map(|broadcast_cred| {
                            let mut metadata = new_metadata.clone();
                            // Only SELF scope credentials should have the stable device ID.
                            if discovery_cred.visibility_scope.as_ref().is_some_and(|scope| {
                                scope.level.enum_value_or_default() == VISIBILITY_LEVEL_SELF_GROUP
                            }) {
                                metadata.device_id = preferences.borrow().device_id.to_vec();
                            }

                            metadata.visibility_scope = discovery_cred.visibility_scope.clone();

                            credentials::replace_credential_encrypted_metadata::<C>(
                                broadcast_cred,
                                discovery_cred,
                                &metadata,
                            )
                        })
                    })
                    .collect();
                scoped_creds.converted_discovery_credentials = updated_discovery_credentials;
            });

            // Save the updated credential book back to storage first.
            credentials_storage
                .insert_local_credentials(provider_name, local_credential_book.clone())?;
            Self::notify_provider_of_local_credentials_change(
                provider_name,
                local_credential_book,
                local_discovery_credentials_changed.clone(),
            );
        }

        info!("Metadata updated and credentials refreshed!");
        Ok(())
    }

    fn extract_unscoped_metadata(new_prefs: IdentityManagerPreferences) -> DeviceIdentityMetaData {
        let device_name = new_prefs.device_name;
        let device_type = new_prefs.device_type;
        let instance_type = new_prefs.instance_type;
        let device_model_name = new_prefs.device_model_name;
        let device_manufacturer = new_prefs.device_manufacturer;

        DeviceIdentityMetaData {
            device_type: DeviceType::from_i32(device_type as i32).unwrap_or_default().into(),
            device_name,
            device_manufacturer,
            device_model_name,
            instance_type: InstanceType::from_i32(instance_type as i32).unwrap_or_default().into(),
            ..Default::default()
        }
    }

    fn verify_credential_validity(
        &self,
        local_credential_book: &LocalCredentialBook,
        provider_name: &ProviderName,
    ) -> Result<(), CredentialsError> {
        for scoped_broadcast_credentials in local_credential_book.local_credentials.values() {
            if !credentials::are_broadcast_credentials_valid(
                &scoped_broadcast_credentials.broadcast_credentials,
            ) {
                self.wipeout_credentials(provider_name, None)?;
                self.replenish_credentials(provider_name, None)?;
                return Err(CredentialsError::InvalidCredentialMaterial);
            }
        }
        Ok(())
    }

    fn local_to_v0_broadcast(
        cred: &BroadcastCredential,
    ) -> Result<V0BroadcastCredential, CredentialsError> {
        let key_seed: [u8; 32] = cred
            .key_seed
            .clone()
            .try_into()
            .map_err(|_| CredentialsError::InvalidBroadcastCredentials)?;
        let identity_token: [u8; V0_IDENTITY_TOKEN_LEN] = cred
            .identity_token_v0
            .clone()
            .try_into()
            .map_err(|_| CredentialsError::InvalidBroadcastCredentials)?;
        Ok(V0BroadcastCredential::new(key_seed, V0IdentityToken::new(identity_token)))
    }

    fn local_to_v1_broadcast(
        cred: &BroadcastCredential,
    ) -> Result<V1BroadcastCredential, CredentialsError> {
        let key_seed: [u8; 32] = cred
            .key_seed
            .clone()
            .try_into()
            .map_err(|_| CredentialsError::InvalidBroadcastCredentials)?;
        let identity_token: [u8; V1_IDENTITY_TOKEN_LEN] = cred
            .identity_token_v1
            .clone()
            .try_into()
            .map_err(|_| CredentialsError::InvalidBroadcastCredentials)?;
        Ok(V1BroadcastCredential::new(key_seed, V1IdentityToken::from(identity_token)))
    }

    fn create_scoped_metadata(
        &self,
        visibility_scope: &VisibilityScope,
        stable_device_id: Option<[u8; DEVICE_ID_LENGTH]>,
    ) -> DeviceIdentityMetaData {
        let mut metadata = self.inner.load().unscoped_metadata.clone();
        metadata.visibility_scope = protobuf::MessageField::some(visibility_scope.clone().into());
        if let Some(id) = stable_device_id {
            metadata.device_id = id.to_vec();
        }
        metadata
    }

    fn notify_provider_of_local_credentials_change(
        provider_name: &ProviderName,
        local_credential_book: LocalCredentialBook,
        local_discovery_credentials_changed: broadcast::Sender<LocalDiscoveryCredentials>,
    ) {
        if let Err(err) = local_discovery_credentials_changed
            .send(LocalDiscoveryCredentials((provider_name.clone(), local_credential_book)))
        {
            log::error!(
                "Failed to send local discovery credentials update to listeners with: {err:?}."
            );
        }
    }

    fn convert_level_int_to_visibility_level(
        value: i32,
    ) -> Result<VisibilityLevel, CredentialsError> {
        match value {
            v if v == VisibilityLevel::SelfScope as i32 => Ok(VisibilityLevel::SelfScope),
            v if v == VisibilityLevel::Family as i32 => Ok(VisibilityLevel::Family),
            v if v == VisibilityLevel::Contact as i32 => Ok(VisibilityLevel::Contact),
            v if v == VisibilityLevel::Public as i32 => Ok(VisibilityLevel::Public),
            _ => Err(CredentialsError::InvalidVisibilityScope),
        }
    }

    fn build_book_from_discovery_credentials(
        credentials: &[DiscoveryCredential],
    ) -> Arc<PrecalculatedOwnedCredentialBookWithId<EncryptedMetadata>> {
        let v0_creds = credentials.iter().filter_map(|cred| {
            let key_seed: [u8; 32] = cred.key_seed.clone().try_into().ok()?;
            let identity_token_hmac_key_v0: [u8; 32] =
                cred.identity_token_hmac_key_v0.clone().try_into().ok()?;
            Some(MatchableCredential {
                discovery_credential: V0DiscoveryCredential::new(
                    key_seed,
                    identity_token_hmac_key_v0,
                ),
                match_data: EncryptedMetadata::new(
                    cred.encrypted_metadata_bytes_v0.clone(),
                    cred.id,
                ),
            })
        });
        let v1_creds = credentials.iter().filter_map(|cred| {
            let key_seed: [u8; 32] = cred.key_seed.clone().try_into().ok()?;
            let identity_token_short_salt_adv_hmac_key: [u8; 32] =
                cred.identity_token_short_salt_adv_hmac_key_v1.clone().try_into().ok()?;
            let identity_token_extended_salt_hmac_key: [u8; 32] =
                cred.identity_token_extended_salt_adv_hmac_key_v1.clone().try_into().ok()?;
            Some(MatchableCredential {
                discovery_credential: V1DiscoveryCredential::new(
                    key_seed,
                    identity_token_short_salt_adv_hmac_key,
                    identity_token_extended_salt_hmac_key,
                ),
                match_data: EncryptedMetadata::new(
                    cred.encrypted_metadata_bytes_v1.clone(),
                    cred.id,
                ),
            })
        });

        Arc::new(CredentialBookBuilder::build_precalculated_owned_book_with_id::<C>(
            v0_creds, v1_creds,
        ))
    }

    fn consume_salt_for_credential(
        &self,
        active_cred: &BroadcastCredential,
    ) -> Result<(V0BroadcastCredential, V0Salt), CredentialsError> {
        let provider_name = ProviderName::default();

        let mut consumed_salts_book =
            self.credentials_storage.get_consumed_salts(&provider_name).unwrap_or_default();

        let mut consumed_salts_for_cred: HashSet<Vec<u8>> = consumed_salts_book
            .consumed_salts_map
            .entry(active_cred.id as u32)
            .or_default()
            .salt
            .iter()
            .cloned()
            .collect();

        if consumed_salts_for_cred.len() >= MAX_NUM_SALTS {
            info!("All salts consumed for V0 credential, regenerating credentials for provider.");
            self.wipeout_credentials(&provider_name, None)?;
            self.replenish_credentials(&provider_name, None)?;
            return Err(CredentialsError::InvalidCredentialMaterial);
        }

        // Generate and add a new salt.
        let mut salt_bytes = [0u8; V0_SALT_LEN];
        let mut rng = C::CryptoRng::new();
        for _ in 0..MAX_NUM_RETRIES {
            rng.fill(&mut salt_bytes);
            let salt_vec = salt_bytes.to_vec();
            if consumed_salts_for_cred.contains(&salt_vec) {
                continue;
            }
            // Found a new salt.
            let _ = consumed_salts_for_cred.insert(salt_vec);

            consumed_salts_book.consumed_salts_map.entry(active_cred.id as u32).or_default().salt =
                consumed_salts_for_cred.into_iter().collect();

            self.credentials_storage.insert_consumed_salts(&provider_name, consumed_salts_book)?;

            let salt = ldt_np_adv::V0Salt::from(salt_bytes);
            let broadcast_credential = Self::local_to_v0_broadcast(active_cred)?;
            return Ok((broadcast_credential, salt));
        }

        Err(CredentialsError::V0ConsumedSaltsFailure)
    }

    fn extract_v1_provider_name(
        provider: &Option<String>,
    ) -> Result<ProviderName, CredentialsError> {
        if let Some(provider_name) = provider {
            return Ok(ProviderName(provider_name.clone()));
        }
        Err(CredentialsError::InvalidVisibilityScope)
    }
}

impl<C> Drop for CredentialManagerImpl<C>
where
    C: CryptoProvider + Send,
    C::Ed25519: Ed25519Provider,
{
    fn drop(&mut self) {
        self.preferences_update_handle.abort();
    }
}

impl<C> CredentialManager for CredentialManagerImpl<C>
where
    C: CryptoProvider + Sync + Send,
    C::Ed25519: Ed25519Provider,
{
    fn get_v0_broadcast_credential_with_salt(
        &self,
        visibility_level: &VisibilityLevel,
    ) -> Result<(V0BroadcastCredential, V0Salt), CredentialsError> {
        // V0 broadcast credentials are only surfaced for a "default" identity provider,
        // configured in /beto-rust/nearby/identity/common/src/definitions.rs.
        // This prevents the system from generating an encrypted legacy advertisement
        // for every registered identity provider.
        let provider_name = ProviderName::default();
        let visibility_key = *visibility_level as i32;

        // Try to get from cache first.
        if let Some(active_cred) = self
            .inner
            .load()
            .broadcast_credentials
            .get(&provider_name)
            .and_then(|provider_creds| provider_creds.get(&visibility_key))
            .and_then(|scoped_creds| credentials::find_active_credential(scoped_creds))
        {
            return self.consume_salt_for_credential(active_cred);
        }

        // If not found, load from storage and replenish cache.
        let local_credential_book =
            self.credentials_storage.get_local_credentials(&provider_name)?;
        self.verify_credential_validity(&local_credential_book, &provider_name)?;

        let _ = self.inner.rcu(|inner| {
            let mut new_inner = inner.as_ref().clone();
            let _ = new_inner.broadcast_credentials.insert(
                provider_name.clone(),
                local_credential_book
                    .local_credentials
                    .clone()
                    .into_iter()
                    .map(|(k, v)| (k, v.broadcast_credentials))
                    .collect(),
            );
            new_inner
        });

        // Try again from replenished cache.
        if let Some(active_cred) = self
            .inner
            .load()
            .broadcast_credentials
            .get(&provider_name)
            .and_then(|provider_creds| provider_creds.get(&visibility_key))
            .and_then(|scoped_creds| credentials::find_active_credential(scoped_creds))
        {
            return self.consume_salt_for_credential(active_cred);
        }

        Err(CredentialsError::CredentialNotFound)
    }

    fn get_v1_broadcast_credential(
        &self,
        visibility_scope: &VisibilityScope,
    ) -> Result<V1BroadcastCredential, CredentialsError> {
        let provider_name = match visibility_scope {
            VisibilityScope::SelfScope { provider } | VisibilityScope::Contact { provider } => {
                Self::extract_v1_provider_name(provider)?
            }
            _ => return Err(CredentialsError::InvalidVisibilityScope),
        };

        let visibility_key = visibility_scope.level() as i32;

        if let Some(active_cred) = self
            .inner
            .load()
            .broadcast_credentials
            .get(&provider_name)
            .and_then(|provider_creds| provider_creds.get(&visibility_key))
            .and_then(|scoped_creds| credentials::find_active_credential(scoped_creds))
        {
            return Self::local_to_v1_broadcast(active_cred);
        }

        // If not found, load from storage and replenish cache.
        let local_credential_book =
            self.credentials_storage.get_local_credentials(&provider_name)?;
        self.verify_credential_validity(&local_credential_book, &provider_name)?;

        let _ = self.inner.rcu(|inner| {
            let mut new_inner = inner.as_ref().clone();
            let _ = new_inner.broadcast_credentials.insert(
                provider_name.clone(),
                local_credential_book
                    .local_credentials
                    .clone()
                    .into_iter()
                    .map(|(k, v)| (k, v.broadcast_credentials))
                    .collect(),
            );
            new_inner
        });

        // Try again from replenished cache.
        if let Some(active_cred) = self
            .inner
            .load()
            .broadcast_credentials
            .get(&provider_name)
            .and_then(|provider_creds| provider_creds.get(&visibility_key))
            .and_then(|scoped_creds| credentials::find_active_credential(scoped_creds))
        {
            return Self::local_to_v1_broadcast(active_cred);
        }

        Err(CredentialsError::CredentialNotFound)
    }

    fn get_private_group_discovery_credential_book(
        &self,
    ) -> Result<Arc<PrecalculatedOwnedCredentialBookWithId<EncryptedMetadata>>, CredentialsError>
    {
        Ok(self.inner.load().private_group_discovery_credential_book.clone())
    }

    fn get_frequent_contact_discovery_credential_book(
        &self,
    ) -> Result<Arc<PrecalculatedOwnedCredentialBookWithId<EncryptedMetadata>>, CredentialsError>
    {
        Ok(self.inner.load().frequent_contact_discovery_credential_book.clone())
    }

    fn get_on_demand_contact_discovery_credential_book(
        &self,
    ) -> Result<crate::credentials::lazy_book::LazyDiscoveryCredentialBook, CredentialsError> {
        Ok(crate::credentials::lazy_book::LazyDiscoveryCredentialBook::new(
            self.credentials_storage.clone(),
            self.inner.load().provider_names.clone(),
        ))
    }

    fn notify_contact_credential_used(&self, id: i64) {
        // Try to find the credential in our frequent list/cache or DB.
        let inner = self.inner.load();
        let existing_index = inner.frequent_contact_credentials.iter().position(|c| c.id == id);

        let credential_to_promote = if let Some(index) = existing_index {
            if index == 0 {
                // Optimization: Already at MRU (index 0).
                // Since the book is built from this vector, and index 0 is checked first,
                // we don't need to do anything.
                return;
            }
            let Some(cred) = inner.frequent_contact_credentials.get(index) else {
                // This should be unreachable since we just found the index,
                // but we return safely to avoid panicking in a rare race condition.
                return;
            };
            cred.clone()
        } else {
            // Not in cache, try to find in DB.
            let found_in_db = self.inner.load().provider_names.iter().find_map(|provider_name| {
                self.credentials_storage.get_remote_credentials(provider_name).ok().and_then(
                    |book| book.contacts_remote_credentials.into_iter().find(|c| c.id == id),
                )
            });

            match found_in_db {
                Some(cred) => cred,
                None => return, // Credential not found anywhere.
            }
        };

        // We have a credential to promote to Index 0.
        let _ = self.inner.rcu(|inner| {
            let mut new_inner = inner.as_ref().clone();

            // Check again inside lock to see if it moved or was added.
            if let Some(current_index) =
                new_inner.frequent_contact_credentials.iter().position(|c| c.id == id)
            {
                if current_index == 0 {
                    return inner.clone();
                }
                let _ = new_inner.frequent_contact_credentials.remove(current_index);
            }

            // Insert at Head (MRU)
            new_inner.frequent_contact_credentials.insert(0, credential_to_promote.clone());

            // Evict LRU (Tail) if full
            if new_inner.frequent_contact_credentials.len() > MAX_CACHED_CONTACT_CREDENTIALS {
                let _ = new_inner.frequent_contact_credentials.pop();
            }

            // Always rebuild the book so the internal pre-calculated book matches the new order.
            new_inner.frequent_contact_discovery_credential_book =
                Self::build_book_from_discovery_credentials(
                    &new_inner.frequent_contact_credentials,
                );
            new_inner.into()
        });
    }

    fn get_signing_credential_id(&self, token: &AuthToken) -> Result<i64, CredentialsError> {
        self.inner
            .load()
            .provider_names
            .iter()
            .find_map(|provider_name| {
                self.credentials_storage.get_local_credentials(provider_name).ok().and_then(
                    |local_credential_book| {
                        local_credential_book.local_credentials.values().find_map(|scoped_creds| {
                            scoped_creds.broadcast_credentials.iter().find_map(|cred| {
                                let local_auth_token =
                                    credentials::extract_auth_token::<C>(&cred.key_seed);
                                if local_auth_token == *token {
                                    Some(cred.id)
                                } else {
                                    None
                                }
                            })
                        })
                    },
                )
            })
            .ok_or(CredentialsError::CredentialNotFound)
    }

    fn get_verifying_discovery_credential(
        &self,
        token: &AuthToken,
    ) -> Result<DiscoveryCredential, CredentialsError> {
        self.inner
            .load()
            .provider_names
            .iter()
            .find_map(|provider_name| {
                self.credentials_storage.get_remote_credentials(provider_name).ok().and_then(
                    |remote_creds_book| {
                        remote_creds_book
                            .self_remote_credentials
                            .iter()
                            .chain(&remote_creds_book.contacts_remote_credentials)
                            .find(|cred| {
                                let local_auth_token =
                                    credentials::extract_auth_token::<C>(&cred.key_seed);
                                local_auth_token == *token
                            })
                            .cloned()
                    },
                )
            })
            .ok_or(CredentialsError::CredentialNotFound)
    }

    fn update_remote_credentials(
        &self,
        provider_name: &ProviderName,
        credentials: Vec<DiscoveryCredential>,
    ) -> Result<(), CredentialsError> {
        let mut self_remote_credentials = Vec::new();
        let mut contacts_remote_credentials = Vec::new();

        // TODO: add support for FAMILY scope remote credentials.
        for cred in credentials {
            let level = cred
                .visibility_scope
                .as_ref()
                .and_then(|s| s.level.enum_value().ok())
                .unwrap_or(VISIBILITY_LEVEL_UNSPECIFIED);
            match level {
                VISIBILITY_LEVEL_SELF_GROUP => self_remote_credentials.push(cred),
                VISIBILITY_LEVEL_CONTACTS_GROUP => contacts_remote_credentials.push(cred),
                _ => {
                    // Unknown or unsupported visibility level, ignore.
                }
            }
        }

        // Create a set of IDs for the new credentials for fast lookup.
        let new_contact_cred_ids: HashSet<i64> =
            contacts_remote_credentials.iter().map(|c| c.id).collect();
        let provider_name_str = provider_name.0.clone();

        let remote_credentials = RemoteCredentialBook {
            self_remote_credentials,
            contacts_remote_credentials,
            ..Default::default()
        };
        self.credentials_storage.insert_remote_credentials(provider_name, remote_credentials).map_err(
            |err| {
                log::error!(
                    "Failed to update remote credentials for provider {provider_name:?} with: {err:?}."
                );
                err
            })?;
        // Reload private group credentials and invalidate quick access cache:
        let private_group_creds = Self::load_all_private_group_credentials(
            &self.inner.load().provider_names,
            self.credentials_storage.as_ref(),
        );
        let private_group_book = Self::build_book_from_discovery_credentials(&private_group_creds);

        let _ = self.inner.rcu(|inner| {
            let mut new_inner = inner.as_ref().clone();
            new_inner.private_group_discovery_credential_book = private_group_book.clone();

            // Prune frequent_contact_credentials associated with this provider
            // that are NOT in the new list.
            new_inner.frequent_contact_credentials.retain(|c| {
                let Some(scope) = c.visibility_scope.as_ref() else {
                    return false;
                };

                if scope.provider_name == provider_name_str {
                    new_contact_cred_ids.contains(&c.id)
                } else {
                    true // Keep credentials from other providers
                }
            });

            new_inner.frequent_contact_discovery_credential_book =
                Self::build_book_from_discovery_credentials(
                    &new_inner.frequent_contact_credentials,
                );

            new_inner
        });
        self.remote_discovery_credentials_changed.send(()).map_err(|err| {
            log::error!("Failed to send remote credentials update to listeners with: {err:?}.");
            err.into()
        })
    }

    fn wipeout_credentials(
        &self,
        provider_name: &ProviderName,
        visibility_level: Option<VisibilityLevel>,
    ) -> Result<(), CredentialsError> {
        match visibility_level {
            None => {
                // Wipeout all credentials for the provider.
                self.credentials_storage.delete_credentials_for_provider(provider_name)?;

                let private_group_creds = Self::load_all_private_group_credentials(
                    &self.inner.load().provider_names,
                    self.credentials_storage.as_ref(),
                );
                let private_group_book =
                    Self::build_book_from_discovery_credentials(&private_group_creds);

                let provider_name_str = provider_name.0.clone();
                let _ = self.inner.rcu(|inner| {
                    let mut new_inner = inner.as_ref().clone();
                    let _ = new_inner.broadcast_credentials.remove(provider_name);
                    new_inner.private_group_discovery_credential_book = private_group_book.clone();

                    // Remove all frequent credentials associated with this provider.
                    new_inner.frequent_contact_credentials.retain(|c| {
                        let Some(scope) = c.visibility_scope.as_ref() else {
                            return false;
                        };

                        scope.provider_name != provider_name_str
                    });

                    new_inner.frequent_contact_discovery_credential_book =
                        Self::build_book_from_discovery_credentials(
                            &new_inner.frequent_contact_credentials,
                        );

                    new_inner
                });
            }
            Some(level) => {
                // Wipeout specific visibility level credentials.
                let mut local_credential_book =
                    self.credentials_storage.get_local_credentials(provider_name)?;
                if local_credential_book.local_credentials.remove(&(level as i32)).is_some() {
                    self.credentials_storage
                        .insert_local_credentials(provider_name, local_credential_book)?;

                    let _ = self.inner.rcu(|inner| {
                        let mut new_inner = inner.as_ref().clone();
                        if let Some(provider_creds) =
                            new_inner.broadcast_credentials.get_mut(provider_name)
                        {
                            let _ = provider_creds.remove(&(level as i32));
                        }
                        new_inner
                    });
                }
            }
        };
        Ok(())
    }

    fn replenish_credentials(
        &self,
        provider_name: &ProviderName,
        visibility_level: Option<VisibilityLevel>,
    ) -> Result<(), CredentialsError> {
        info!("Replenishing credentials for provider: {provider_name:?}");
        let mut local_credentials = self
            .credentials_storage
            .get_local_credentials(provider_name)
            .unwrap_or_else(|err| {
                info!("Failed to load local credentials for {provider_name:?} with {err:?}, replenishing from scratch.");
                LocalCredentialBook::default()
            })
            .local_credentials.clone(); // We clone to prevent mutating credentials on disk.

        let levels_to_replenish = match visibility_level {
            Some(level) => vec![level],
            // If no level is specified, replenish all supported visibility levels.
            None => self
                .preferences
                .borrow()
                .provider_visibilities
                .get(&provider_name.0)
                .map(|levels| {
                    levels
                        .iter()
                        .filter_map(|level_int| {
                            Self::convert_level_int_to_visibility_level(*level_int).ok()
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|| vec![VisibilityLevel::SelfScope, VisibilityLevel::Contact]),
        };

        for visibility_level in levels_to_replenish {
            let broadcast_credentials =
                local_credentials.entry(visibility_level as i32).or_default();

            let mut start_time_millis = Utc::now().timestamp_millis();

            let mut valid_broadcast_creds = Vec::new();
            let mut valid_discovery_creds = Vec::new();

            for (local, shared) in broadcast_credentials
                .broadcast_credentials
                .iter()
                .zip(broadcast_credentials.converted_discovery_credentials.iter())
            {
                if !credentials::is_expired_credential(local) {
                    valid_broadcast_creds.push(local.clone());
                    valid_discovery_creds.push(shared.clone());
                    start_time_millis = max(start_time_millis, local.end_time_millis);
                }
            }

            broadcast_credentials.broadcast_credentials = valid_broadcast_creds;
            broadcast_credentials.converted_discovery_credentials = valid_discovery_creds;

            let credential_count = broadcast_credentials.broadcast_credentials.len();

            if EXPECTED_NUM_LOCAL_CREDENTIALS > credential_count {
                // TODO: log metrics
            }

            let (visibility_scope, stable_device_id) = match visibility_level {
                // Only SELF scope credentials should have the stable device ID.
                VisibilityLevel::SelfScope => (
                    VisibilityScope::SelfScope { provider: Some(provider_name.0.clone()) },
                    Some(self.preferences.borrow().device_id),
                ),
                VisibilityLevel::Family => {
                    (VisibilityScope::Family { provider: Some(provider_name.0.clone()) }, None)
                }
                VisibilityLevel::Contact => {
                    (VisibilityScope::Contact { provider: Some(provider_name.0.clone()) }, None)
                }
                _ => {
                    log::warn!("Unsupported non-public visibility level: {visibility_level:?}");
                    continue;
                }
            };

            for _ in credential_count..EXPECTED_NUM_LOCAL_CREDENTIALS {
                let start_time = match Utc.timestamp_millis_opt(start_time_millis).single() {
                    Some(time) => time,
                    None => {
                        log::error!("Failed to create a DateTime<Utc> from start_time_millis!");
                        break;
                    }
                };
                let device_identity_metadata =
                    self.create_scoped_metadata(&visibility_scope, stable_device_id);
                match credentials::create_credential_pair::<C>(
                    start_time,
                    &device_identity_metadata,
                ) {
                    Ok((broadcast_credential, discovery_credential)) => {
                        start_time_millis = broadcast_credential.end_time_millis;
                        broadcast_credentials.broadcast_credentials.push(broadcast_credential);
                        broadcast_credentials
                            .converted_discovery_credentials
                            .push(discovery_credential);
                    }
                    Err(err) => {
                        log::error!("Failed to generate credential pair with: {err}");
                    }
                }
            }
        }

        let local_credential_book = LocalCredentialBook { local_credentials, ..Default::default() };
        Self::notify_provider_of_local_credentials_change(
            provider_name,
            local_credential_book.clone(),
            self.local_discovery_credentials_changed.clone(),
        );
        Ok(())
    }

    fn save_primed_credentials(
        &self,
        provider_name: &ProviderName,
        local_credential_book: LocalCredentialBook,
    ) -> Result<(), CredentialsError> {
        // Update the cached broadcast credentials.
        let _ = self.inner.rcu(|inner| {
            let mut new_inner = inner.as_ref().clone();
            let _ = new_inner.broadcast_credentials.insert(
                provider_name.clone(),
                local_credential_book
                    .local_credentials
                    .clone()
                    .into_iter()
                    .map(|(k, v)| (k, v.broadcast_credentials))
                    .collect(),
            );
            new_inner
        });

        self.credentials_storage
            .insert_local_credentials(provider_name, local_credential_book)
            .map_err(|err| {
                log::error!(
                    "Failed to save updated local credentials for provider {provider_name:?} with: {err:?}."
                );
                err.into()
            })
    }

    fn subscribe_broadcast_credential_changes(&self) -> broadcast::Receiver<VisibilityScope> {
        self.broadcast_credentials_changed.subscribe()
    }

    fn subscribe_local_discovery_credential_changes(
        &self,
    ) -> broadcast::Receiver<LocalDiscoveryCredentials> {
        self.local_discovery_credentials_changed.subscribe()
    }

    fn subscribe_remote_discovery_credential_changes(&self) -> watch::Receiver<()> {
        self.remote_discovery_credentials_changed.subscribe()
    }

    fn get_visibility_scope_from_id(&self, id: i64) -> Option<VisibilityScope> {
        self.inner.load().provider_names.iter().find_map(|provider_name| {
            self.credentials_storage.get_remote_credentials(provider_name).ok().and_then(
                |remote_creds_book| {
                    remote_creds_book
                        .self_remote_credentials
                        .iter()
                        .chain(remote_creds_book.contacts_remote_credentials.iter())
                        .find(|cred| cred.id == id)
                        .map(|cred| cred.visibility_scope.clone().unwrap_or_default().into())
                },
            )
        })
    }
}
