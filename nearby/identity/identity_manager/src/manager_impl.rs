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
    api::IdentityManager,
    credentials::{
        credential_manager::{CredentialManager, LocalDiscoveryCredentials},
        credential_manager_impl::CredentialManagerImpl,
    },
    errors::IdentityManagerError,
    identity_storage::{
        create_preferences_manager, create_preferences_storage,
        credentials_database::{
            create_credentials_database, create_database_storage, CredentialsDatabase,
        },
        IdentityManagerPreferences, PreferencesManager, DEVICE_ID_LENGTH,
    },
    types::{BroadcastSessionId, StableDeviceId, BROADCAST_SESSION_ID_SIZE},
};
use async_trait::async_trait;
use credential_proto::proto::credential::DiscoveryCredential;
use crypto_provider::{CryptoProvider, CryptoRng};
use identity_common::definitions::{
    DeviceInfo, DeviceType, IdentityProviderConfig, ProviderName, VisibilityLevel, VisibilityScope,
};
use identity_provider::api::{IdentityProvider, IdentityProviderEvent};
use std::sync::RwLock;
use std::{collections::HashMap, marker::PhantomData, sync::Arc};
use storage::{DatabaseStorage, PreferenceKeyValueStorage};
use tokio::{
    sync::{broadcast, mpsc, watch, Mutex},
    task::JoinHandle,
};

pub(crate) struct IdentityManagerImpl<C: CryptoProvider> {
    // Pub crate credential_manager for usage by auth implementation outside of this file.
    pub(crate) credential_manager: Arc<dyn CredentialManager + Send + Sync>,
    providers: Arc<RwLock<HashMap<ProviderName, Arc<dyn IdentityProvider>>>>,
    provider_tasks: Arc<RwLock<HashMap<ProviderName, JoinHandle<()>>>>,
    preferences: Arc<Mutex<dyn PreferencesManager>>,
    preferences_reader: watch::Receiver<IdentityManagerPreferences>,
    local_broadcast_session_id: Arc<RwLock<BroadcastSessionId>>,
    credentials_changed_loop: JoinHandle<()>,
    credential_update_tasks: Arc<RwLock<HashMap<ProviderName, JoinHandle<()>>>>,
    crypto_provider: PhantomData<C>,
}

impl<C: CryptoProvider + Send + Sync + 'static> IdentityManagerImpl<C> {
    pub fn new() -> Result<Self, IdentityManagerError> {
        let preferences_storage = create_preferences_storage()?;
        let credentials_storage = create_database_storage()?;

        Self::new_with_storage(preferences_storage, credentials_storage)
    }

    pub fn new_with_storage<S, D>(
        preferences_storage: S,
        credentials_storage: D,
    ) -> Result<Self, IdentityManagerError>
    where
        S: PreferenceKeyValueStorage + 'static,
        D: DatabaseStorage + 'static,
    {
        let (preferences, preferences_reader) = create_preferences_manager(preferences_storage);
        let credentials_database: Arc<dyn CredentialsDatabase> =
            create_credentials_database(credentials_storage);

        let credential_manager: Arc<dyn CredentialManager + Send + Sync> = Arc::new(
            CredentialManagerImpl::<C>::new(preferences_reader.clone(), credentials_database)?,
        );
        let providers = Arc::new(RwLock::new(HashMap::new()));
        let provider_tasks = Arc::new(RwLock::new(HashMap::new()));
        let credential_update_tasks = Arc::new(RwLock::new(HashMap::new()));

        // Run self discovery credentials changed event loop: (unchanged)
        let local_discovery_credentials_changed_rx =
            credential_manager.subscribe_local_discovery_credential_changes();
        let cloned_providers = providers.clone();
        let cloned_cred_manager = credential_manager.clone();
        let cloned_credential_update_tasks = credential_update_tasks.clone();
        let credentials_changed_loop = tokio::spawn(async move {
            Self::run_credentials_changed_loop(
                local_discovery_credentials_changed_rx,
                cloned_providers,
                cloned_credential_update_tasks,
                cloned_cred_manager,
            )
            .await;
        });
        let local_broadcast_session_id =
            Arc::new(RwLock::new(Self::generate_broadcast_session_id()));

        Ok(Self {
            providers,
            provider_tasks,
            preferences,
            preferences_reader,
            local_broadcast_session_id,
            credential_manager,
            credentials_changed_loop,
            credential_update_tasks,
            crypto_provider: PhantomData,
        })
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_for_test(
        preferences: Arc<Mutex<dyn PreferencesManager>>,
        preferences_reader: watch::Receiver<IdentityManagerPreferences>,
        credential_manager: Arc<dyn CredentialManager + Send + Sync>,
        local_discovery_credentials_changed: broadcast::Sender<LocalDiscoveryCredentials>,
    ) -> Self {
        let providers = Arc::new(RwLock::new(HashMap::new()));
        let provider_tasks = Arc::new(RwLock::new(HashMap::new()));
        let credential_update_tasks = Arc::new(RwLock::new(HashMap::new()));

        let cloned_providers = providers.clone();
        let cloned_cred_manager = credential_manager.clone();
        let cloned_credential_update_tasks = credential_update_tasks.clone();
        let credentials_changed_loop = tokio::spawn(async move {
            Self::run_credentials_changed_loop(
                local_discovery_credentials_changed.subscribe(),
                cloned_providers,
                cloned_credential_update_tasks,
                cloned_cred_manager,
            )
            .await;
        });
        let local_broadcast_session_id =
            Arc::new(RwLock::new(Self::generate_broadcast_session_id()));

        Self {
            providers,
            provider_tasks,
            preferences,
            preferences_reader,
            local_broadcast_session_id,
            credential_manager,
            credentials_changed_loop,
            credential_update_tasks,
            crypto_provider: PhantomData,
        }
    }

    fn generate_device_id() -> [u8; DEVICE_ID_LENGTH] {
        let mut rng = C::CryptoRng::new();
        let mut bytes = [0u8; DEVICE_ID_LENGTH];
        rng.fill(&mut bytes);
        bytes
    }

    fn generate_broadcast_session_id() -> BroadcastSessionId {
        let mut rng = C::CryptoRng::new();
        let mut bytes = [0u8; BROADCAST_SESSION_ID_SIZE];
        rng.fill(&mut bytes);
        BroadcastSessionId::new(bytes)
    }

    #[allow(clippy::expect_used)]
    pub(crate) fn rotate_local_broadcast_session_id(&self) {
        let new_broadcast_session_id = Self::generate_broadcast_session_id();
        // SAFETY: A poisoned lock is a panic condition and indicates a bug.
        *self
            .local_broadcast_session_id
            .write()
            .expect("RwLock for local_broadcast_session_id was poisoned") =
            new_broadcast_session_id;
    }

    pub(crate) fn get_local_broadcast_session_id(&self) -> BroadcastSessionId {
        // SAFETY: A poisoned lock is a panic condition and indicates a bug.
        #[allow(clippy::expect_used)]
        *self
            .local_broadcast_session_id
            .read()
            .expect("RwLock for local_broadcast_session_id was poisoned")
    }

    pub(crate) fn preferences(&self) -> watch::Receiver<IdentityManagerPreferences> {
        self.preferences_reader.clone()
    }

    // Identity provider unregistration logic: This runs when a channel closes.
    async fn execute_unregister_logic(
        provider_name: ProviderName,
        providers: Arc<RwLock<HashMap<ProviderName, Arc<dyn IdentityProvider>>>>,
        preferences: Arc<Mutex<dyn PreferencesManager>>,
        credential_manager: Arc<dyn CredentialManager + Send + Sync>,
    ) {
        log::info!("Unregistering identity provider: {provider_name:?}");
        // 1. Remove provider from the active map
        // SAFETY: A poisoned lock is a panic condition and indicates a bug.
        #[allow(clippy::expect_used)]
        let _ =
            providers.write().expect("RwLock for providers was poisoned").remove(&provider_name);

        // 2. Handle device ID and preferences cleanup
        let mut prefs_lock = preferences.lock().await;
        // SAFETY: A poisoned lock is a panic condition and indicates a bug.
        #[allow(clippy::expect_used)]
        if providers.read().expect("RwLock for providers was poisoned").is_empty() {
            log::info!("All providers have been unregistered, wiping out device ID.");
            if let Err(err) = prefs_lock.set_device_id([0; DEVICE_ID_LENGTH]) {
                log::error!("Failed to wipeout device ID with: {err}");
            }
        }

        let mut provider_visibilities =
            prefs_lock.subscribe().borrow().provider_visibilities.clone();
        let _ = provider_visibilities.remove(&provider_name.0);
        if let Err(err) = prefs_lock.set_provider_visibilities(provider_visibilities) {
            log::error!("Failed to update provider visibilities: {err}");
        }

        // 3. Wipeout credentials
        let _ = credential_manager.wipeout_credentials(&provider_name, None).map_err(|err| {
            log::error!("Failed to wipeout credentials for {provider_name:?} with: {err}.");
        });

        log::info!("Successfully executed unregister logic for provider: {}", provider_name.0);
    }

    async fn run_credentials_changed_loop(
        mut local_discovery_credentials_changed_rx: broadcast::Receiver<LocalDiscoveryCredentials>,
        providers: Arc<RwLock<HashMap<ProviderName, Arc<dyn IdentityProvider>>>>,
        credential_update_tasks: Arc<RwLock<HashMap<ProviderName, JoinHandle<()>>>>,
        credential_manager: Arc<dyn CredentialManager + Send + Sync>,
    ) {
        loop {
            tokio::select! {
                Ok(local_credentials) = local_discovery_credentials_changed_rx.recv() => {
                    let (provider_name, local_credential_book) = local_credentials.0;

                    let provider = {
                        // SAFETY: A poisoned lock is a panic condition and indicates a bug.
                        #[allow(clippy::expect_used)]
                        providers.read().expect("RwLock for providers was poisoned").get(&provider_name).cloned()
                    };

                    if let Some(provider) = provider {
                        let cloned_cred_manager = credential_manager.clone();
                        let cloned_provider_name = provider_name.clone();
                        let task = tokio::spawn(async move {
                            // Convert the book to the format expected by the provider
                            let discovery_credentials: HashMap<VisibilityLevel, Vec<DiscoveryCredential>> =
                                local_credential_book
                                    .local_credentials
                                    .iter()
                                    .filter_map(|(level_int, creds)| {
                                        let level = match *level_int {
                                            v if v == VisibilityLevel::SelfScope as i32 => VisibilityLevel::SelfScope,
                                            v if v == VisibilityLevel::Family as i32 => VisibilityLevel::Family,
                                            v if v == VisibilityLevel::Contact as i32 => VisibilityLevel::Contact,
                                            v if v == VisibilityLevel::Public as i32 => VisibilityLevel::Public,
                                            _ => return None,
                                        };
                                        Some((level, creds.converted_discovery_credentials.clone()))
                                    })
                                    .collect();

                            provider.update_local_discovery_credentials(discovery_credentials).await;

                            if let Err(err) = cloned_cred_manager.save_primed_credentials(&cloned_provider_name, local_credential_book) {
                                log::error!("Failed to save primed credentials for {cloned_provider_name:?} with: {err:?}");
                            }
                        });
                        // Store the join handle to the task
                        // SAFETY: A poisoned lock is a panic condition and indicates a bug.
                        #[allow(clippy::expect_used)]
                        let mut tasks =
                            credential_update_tasks.write().expect("RwLock for credential_update_tasks was poisoned");
                        let _ = tasks.insert(provider_name, task);
                    }
                }
            }
        }
    }
}

impl<C: CryptoProvider> Drop for IdentityManagerImpl<C> {
    fn drop(&mut self) {
        // SAFETY: A poisoned lock is a panic condition and indicates a bug.
        #[allow(clippy::expect_used)]
        let tasks = self.provider_tasks.read().expect("RwLock for provider_tasks was poisoned");
        for task in tasks.values() {
            task.abort();
        }
        self.credentials_changed_loop.abort();
        // SAFETY: A poisoned lock is a panic condition and indicates a bug.
        #[allow(clippy::expect_used)]
        let tasks = self
            .credential_update_tasks
            .read()
            .expect("RwLock for credential_update_tasks was poisoned");
        for task in tasks.values() {
            task.abort();
        }
    }
}

#[async_trait]
impl<C: CryptoProvider + Send + Sync + 'static> IdentityManager for IdentityManagerImpl<C> {
    fn register_identity_provider(
        &self,
        mut provider: Box<dyn IdentityProvider>,
        config: IdentityProviderConfig,
    ) {
        log::info!("Registering identity provider with name: {:?}", config.provider_name);
        let (identity_provider_event_tx, mut identity_provider_event_rx) = mpsc::channel(10);
        let cloned_providers = self.providers.clone();
        let cloned_preferences = self.preferences.clone();
        let cloned_cred_manager = self.credential_manager.clone();
        let provider_name = config.provider_name.clone();
        let provider_name_for_map = provider_name.clone();

        let task = tokio::spawn(async move {
            // SAFETY: A poisoned lock is a panic condition and indicates a bug.
            #[allow(clippy::expect_used)]
            if cloned_providers.read().expect("RwLock for providers was poisoned").is_empty() {
                log::info!("First provider has been registered, generating device ID.");
                let device_id = Self::generate_device_id();
                if let Err(err) = cloned_preferences.lock().await.set_device_id(device_id) {
                    log::error!(
                        "Failed to set device id with: {err}, aborting provider registration."
                    );
                    return;
                }
            }

            {
                let mut prefs_lock = cloned_preferences.lock().await;
                let mut provider_visibilities =
                    prefs_lock.subscribe().borrow().provider_visibilities.clone();
                let _ = provider_visibilities.insert(
                    config.provider_name.0.clone(),
                    config
                        .visibility_levels
                        .iter()
                        .filter(|v| **v != VisibilityLevel::Public)
                        .map(|v| *v as i32)
                        .collect(),
                );
                if let Err(err) = prefs_lock.set_provider_visibilities(provider_visibilities) {
                    log::error!(
                        "Failed to set provider visibilities with: {err}, aborting provider registration."
                    );
                    return;
                }
                let _ = cloned_cred_manager.replenish_credentials(&provider_name, None).map_err(
                    |err| {
                        log::error!(
                            "Failed to replenish credentials for {provider_name:?} with: {err}."
                        );
                    },
                );
            }

            // Pass the sender to the IdentityProvider so it can send events
            provider.on_provider_registered(identity_provider_event_tx).await;
            // SAFETY: A poisoned lock is a panic condition and indicates a bug.
            #[allow(clippy::expect_used)]
            let _ = cloned_providers
                .write()
                .expect("RwLock for providers was poisoned")
                .insert(config.provider_name.clone(), Arc::from(provider));
            log::info!("Successfully registered provider: {}", config.provider_name.0);

            // Start polling for events
            while let Some(event) = identity_provider_event_rx.recv().await {
                match event {
                    IdentityProviderEvent::RemoteCredentialsSynced { remote_credentials } => {
                        if let Err(err) = cloned_cred_manager
                            .update_remote_credentials(&provider_name, remote_credentials)
                        {
                            log::error!("Failed to update remote credentials for {provider_name:?} with: {err}");
                        };
                    }
                }
            }

            // Channel closed, execute identity provider unregistration logic
            log::info!("Identity Provider ({provider_name:?}) receiver closed. Executing unregister logic.");
            Self::execute_unregister_logic(
                provider_name,
                cloned_providers,
                cloned_preferences,
                cloned_cred_manager,
            )
            .await;
        });

        // Store the join handle to the task
        // SAFETY: A poisoned lock is a panic condition and indicates a bug.
        #[allow(clippy::expect_used)]
        let mut tasks =
            self.provider_tasks.write().expect("RwLock for provider_tasks was poisoned");
        let _ = tasks.insert(provider_name_for_map, task);
    }

    async fn rotate_credentials(
        &self,
        provider_name: ProviderName,
        visibility_level: VisibilityLevel,
    ) -> Result<(), IdentityManagerError> {
        log::info!("Rotating credentials for {provider_name:?} with {visibility_level:?}");
        let mut prefs_lock = self.preferences.lock().await;

        if visibility_level == VisibilityLevel::SelfScope {
            log::info!(
                "Account has been opted out of cross-device settings, regenerating the device ID and rotating SELF scope credentials for all providers."
            );
            let device_id = Self::generate_device_id();
            if let Err(err) = prefs_lock.set_device_id(device_id) {
                log::error!("Failed to set device id with: {err}");
            }
            // Regenerate SELF scope credentials for all providers.
            // SAFETY: A poisoned lock is a panic condition and indicates a bug.
            #[allow(clippy::expect_used)]
            for name in self.providers.read().expect("RwLock for providers was poisoned").keys() {
                let _ = self
                    .credential_manager
                    .wipeout_credentials(name, Some(VisibilityLevel::SelfScope))
                    .map_err(|err| {
                        log::error!("Failed to wipeout credentials for {name:?} with: {err}.");
                    });
                let _ = self
                    .credential_manager
                    .replenish_credentials(name, Some(VisibilityLevel::SelfScope))?;
            }
        } else {
            // Regenerate credentials for provider + visibility level.
            let _ = self
                .credential_manager
                .wipeout_credentials(&provider_name, Some(visibility_level))?;
            let _ = self
                .credential_manager
                .replenish_credentials(&provider_name, Some(visibility_level))?;
        }
        Ok(())
    }

    async fn update_device_info(
        &self,
        device_info: DeviceInfo,
    ) -> Result<(), IdentityManagerError> {
        log::info!("Updating device info!");
        // TODO: extract device photo from DeviceInfo and store it in preferences.
        let mut prefs_lock = self.preferences.lock().await;
        let device_name = device_info.device_name;
        Ok(prefs_lock.set_device_name(device_name)?)
    }

    fn local_device_name(&self) -> String {
        self.preferences().borrow().device_name.clone()
    }

    fn local_device_type(&self) -> DeviceType {
        self.preferences().borrow().device_type.try_into().unwrap_or_default()
    }

    fn local_device_id(&self) -> StableDeviceId {
        StableDeviceId::new(self.preferences_reader.borrow().device_id)
    }

    fn rotate_broadcast_session_id(&self) {
        self.rotate_local_broadcast_session_id()
    }

    fn local_broadcast_session_id(&self) -> BroadcastSessionId {
        self.get_local_broadcast_session_id()
    }

    fn encrypted_local_identity(&self) -> Option<Vec<u8>> {
        // TODO: implement encrypted local identity
        None
    }

    fn get_v0_broadcast_credential_with_salt(
        &self,
        visibility: &identity_common::definitions::VisibilityLevel,
    ) -> Result<
        (up_adv::credential::v0::V0BroadcastCredential, ldt_np_adv::V0Salt),
        crate::errors::CredentialsError,
    > {
        self.credential_manager.get_v0_broadcast_credential_with_salt(visibility)
    }

    fn get_v1_broadcast_credential(
        &self,
        visibility: &identity_common::definitions::VisibilityScope,
    ) -> Result<up_adv::credential::v1::V1BroadcastCredential, crate::errors::CredentialsError>
    {
        self.credential_manager.get_v1_broadcast_credential(visibility)
    }

    fn get_private_group_discovery_credential_book(
        &self,
    ) -> Result<
        std::sync::Arc<
            up_adv::credential::book::PrecalculatedOwnedCredentialBookWithId<
                crate::credentials::EncryptedMetadata,
            >,
        >,
        crate::errors::CredentialsError,
    > {
        self.credential_manager.get_private_group_discovery_credential_book()
    }

    fn get_frequent_contact_discovery_credential_book(
        &self,
    ) -> Result<
        std::sync::Arc<
            up_adv::credential::book::PrecalculatedOwnedCredentialBookWithId<
                crate::credentials::EncryptedMetadata,
            >,
        >,
        crate::errors::CredentialsError,
    > {
        self.credential_manager.get_frequent_contact_discovery_credential_book()
    }

    fn get_on_demand_contact_discovery_credential_book(
        &self,
    ) -> Result<
        crate::credentials::lazy_book::LazyDiscoveryCredentialBook,
        crate::errors::CredentialsError,
    > {
        self.credential_manager.get_on_demand_contact_discovery_credential_book()
    }

    fn notify_contact_credential_used(&self, id: i64) {
        self.credential_manager.notify_contact_credential_used(id)
    }

    fn get_visibility_scope_from_id(
        &self,
        id: i64,
    ) -> Option<identity_common::definitions::VisibilityScope> {
        self.credential_manager.get_visibility_scope_from_id(id)
    }

    async fn broadcast_credentials_changed(&self) -> Result<VisibilityScope, IdentityManagerError> {
        let mut receiver = self.credential_manager.subscribe_broadcast_credential_changes();
        receiver.recv().await.map(Ok).unwrap_or(Err(IdentityManagerError::ChannelDropped))
    }

    async fn discovery_credentials_changed(&self) -> Result<(), IdentityManagerError> {
        let mut receiver = self.credential_manager.subscribe_remote_discovery_credential_changes();
        receiver.changed().await.map_err(|_| IdentityManagerError::ChannelDropped)?;
        let _ = receiver.borrow_and_update();
        Ok(())
    }
}
