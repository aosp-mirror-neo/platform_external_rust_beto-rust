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
    errors::IdentityManagerError,
    identity_storage::{
        create_preferences_manager, create_preferences_storage, IdentityManagerPreferences,
        PreferencesManager,
    },
    types::{BroadcastSessionId, StableDeviceId, BROADCAST_SESSION_ID_SIZE},
};
use async_trait::async_trait;
use crypto_provider::{CryptoProvider, CryptoRng};
use identity_common::definitions::{DeviceInfo, DeviceType, VisibilityScope};
use std::{
    marker::PhantomData,
    sync::{Arc, RwLock},
};
use storage::PreferenceKeyValueStorage;
use tokio::sync::{watch, Mutex};

pub struct IdentityManagerImpl<C: CryptoProvider> {
    preferences: Arc<Mutex<dyn PreferencesManager>>,
    preferences_reader: watch::Receiver<IdentityManagerPreferences>,
    pub(crate) local_broadcast_session_id: Arc<RwLock<BroadcastSessionId>>,
    crypto_provider: PhantomData<C>,
}

impl<C: CryptoProvider + Send + Sync> IdentityManagerImpl<C> {
    pub fn new() -> Result<Self, IdentityManagerError> {
        let preferences_storage = create_preferences_storage()?;
        Self::new_with_storage(preferences_storage)
    }

    pub fn new_with_storage<S>(preferences_storage: S) -> Result<Self, IdentityManagerError>
    where
        S: PreferenceKeyValueStorage + 'static,
    {
        let (preferences, preferences_reader) = create_preferences_manager(preferences_storage);
        let local_broadcast_session_id =
            Arc::new(RwLock::new(Self::generate_broadcast_session_id()));

        Ok(Self {
            preferences,
            preferences_reader,
            local_broadcast_session_id,
            crypto_provider: PhantomData,
        })
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
}

#[async_trait]
impl<C: CryptoProvider + Send + Sync + 'static> IdentityManager for IdentityManagerImpl<C> {
    async fn update_device_info(
        &self,
        device_info: DeviceInfo,
    ) -> Result<(), IdentityManagerError> {
        // TODO: extract device photo from DeviceInfo and store it in preferences.
        let mut prefs_lock = self.preferences.lock().await;
        let device_name = device_info.device_name;
        Ok(prefs_lock.set_device_name(device_name)?)
    }

    fn local_device_name(&self) -> String {
        self.preferences().borrow().device_name.clone()
    }

    fn local_device_type(&self) -> DeviceType {
        #[allow(clippy::expect_used)]
        up_adv::shared_data::DeviceType::try_from(self.preferences().borrow().device_type)
            .expect("Device type should be valid.")
    }

    fn local_device_id(&self) -> StableDeviceId {
        StableDeviceId::new(self.preferences().borrow().device_id)
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

    async fn broadcast_credentials_changed(&self) -> Result<VisibilityScope, IdentityManagerError> {
        // Public implementation does not have credentials.
        Err(IdentityManagerError::NotImplemented)
    }

    async fn discovery_credentials_changed(&self) -> Result<(), IdentityManagerError> {
        // Public implementation does not have credentials.
        Err(IdentityManagerError::NotImplemented)
    }
}
