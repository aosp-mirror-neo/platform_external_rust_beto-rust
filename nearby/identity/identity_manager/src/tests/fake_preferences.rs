#![allow(clippy::unwrap_used)]
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

use crate::identity_storage::{
    DeviceManufacturer, DeviceModelName, DeviceType, IdentityManagerPreferences, InstanceType,
    PreferencesManager, DEVICE_ID_LENGTH,
};
use std::collections::HashMap;
use storage::{Key, StorageError};
use tokio::sync::watch;

/// A fake `PreferencesManager` for testing that stores data in memory and
/// notifies subscribers of changes.
#[allow(dead_code)]
pub(crate) struct FakePreferencesManager {
    watcher: watch::Sender<IdentityManagerPreferences>,
    _receiver: watch::Receiver<IdentityManagerPreferences>,
}

impl Default for FakePreferencesManager {
    fn default() -> Self {
        let (sender, receiver) = watch::channel(IdentityManagerPreferences::default());
        Self { watcher: sender, _receiver: receiver }
    }
}

impl PreferencesManager for FakePreferencesManager {
    fn set_device_name(&mut self, value: String) -> Result<(), StorageError> {
        let mut prefs = self.watcher.borrow().clone();
        prefs.device_name = value;
        let _ = self.watcher.send(prefs); // Ignore error in test fake
        Ok(())
    }

    fn set_device_type(&mut self, value: <DeviceType as Key>::Value) -> Result<(), StorageError> {
        let mut prefs = self.watcher.borrow().clone();
        prefs.device_type = value;
        let _ = self.watcher.send(prefs);
        Ok(())
    }

    fn set_device_id(&mut self, value: [u8; DEVICE_ID_LENGTH]) -> Result<(), StorageError> {
        let mut prefs = self.watcher.borrow().clone();
        prefs.device_id = value;
        let _ = self.watcher.send(prefs);
        Ok(())
    }

    fn set_instance_type(
        &mut self,
        value: <InstanceType as Key>::Value,
    ) -> Result<(), StorageError> {
        let mut prefs = self.watcher.borrow().clone();
        prefs.instance_type = value;
        let _ = self.watcher.send(prefs);
        Ok(())
    }

    fn set_device_model_name(
        &mut self,
        value: <DeviceModelName as Key>::Value,
    ) -> Result<(), StorageError> {
        let mut prefs = self.watcher.borrow().clone();
        prefs.device_model_name = value;
        let _ = self.watcher.send(prefs);
        Ok(())
    }

    fn set_device_manufacturer(
        &mut self,
        value: <DeviceManufacturer as Key>::Value,
    ) -> Result<(), StorageError> {
        let mut prefs = self.watcher.borrow().clone();
        prefs.device_manufacturer = value;
        let _ = self.watcher.send(prefs);
        Ok(())
    }

    fn set_provider_visibilities(
        &mut self,
        value: HashMap<String, Vec<i32>>,
    ) -> Result<(), StorageError> {
        let mut prefs = self.watcher.borrow().clone();
        prefs.provider_visibilities = value;
        let _ = self.watcher.send(prefs);
        Ok(())
    }

    fn subscribe(&self) -> watch::Receiver<IdentityManagerPreferences> {
        self.watcher.subscribe()
    }
}
