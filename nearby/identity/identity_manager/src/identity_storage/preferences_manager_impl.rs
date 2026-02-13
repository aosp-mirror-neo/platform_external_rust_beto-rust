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
    DeviceId, DeviceManufacturer, DeviceModelName, DeviceName, DeviceType,
    IdentityManagerPreference, IdentityManagerPreferences, InstanceType, PreferencesManager,
    ProviderVisibilities, DEVICE_ID_LENGTH, IDENTITY_PREFERENCES_FILE_PATH,
};
use std::{
    collections::{HashMap, HashSet},
    fs,
};
use storage::{Key, PreferenceKeyValueStorage, StorageError};
use tokio::sync::watch::{Receiver, Sender};

pub(crate) struct PreferencesManagerImpl<S: PreferenceKeyValueStorage> {
    watcher: Sender<IdentityManagerPreferences>,
    repository: S,
}

macro_rules! define_setter {
    // This branch handles types that need to be cloned (e.g., String, HashMap)
    ($method_name:ident, $pref_type:ty, $field:ident, $value_type:ty, clone) => {
        fn $method_name(&mut self, value: $value_type) -> Result<(), StorageError> {
            self.set::<$pref_type>(value.clone()).map(|_| {
                let _ = self.watcher.send_if_modified(|preferences| {
                    if preferences.$field != value {
                        preferences.$field = value;
                        true
                    } else {
                        false
                    }
                });
            })
        }
    };
    // This branch handles types that are `Copy` (e.g., u8, u128)
    ($method_name:ident, $pref_type:ty, $field:ident, $value_type:ty) => {
        fn $method_name(&mut self, value: $value_type) -> Result<(), StorageError> {
            self.set::<$pref_type>(value).map(|_| {
                let _ = self.watcher.send_if_modified(|preferences| {
                    if preferences.$field != value {
                        preferences.$field = value;
                        true
                    } else {
                        false
                    }
                });
            })
        }
    };
}

impl<S: PreferenceKeyValueStorage> PreferencesManager for PreferencesManagerImpl<S> {
    fn subscribe(&self) -> Receiver<IdentityManagerPreferences> {
        self.watcher.subscribe()
    }

    define_setter!(set_device_name, DeviceName, device_name, <DeviceName as Key>::Value, clone);
    define_setter!(set_device_type, DeviceType, device_type, <DeviceType as Key>::Value);
    define_setter!(set_instance_type, InstanceType, instance_type, <InstanceType as Key>::Value);
    define_setter!(
        set_device_model_name,
        DeviceModelName,
        device_model_name,
        <DeviceModelName as Key>::Value,
        clone
    );
    define_setter!(
        set_device_manufacturer,
        DeviceManufacturer,
        device_manufacturer,
        <DeviceManufacturer as Key>::Value,
        clone
    );
    fn set_device_id(&mut self, value: [u8; DEVICE_ID_LENGTH]) -> Result<(), StorageError> {
        self.set::<DeviceId>(u128::from_be_bytes(value)).map(|_| {
            let _ = self.watcher.send_if_modified(|preferences| {
                if preferences.device_id != value {
                    preferences.device_id = value;
                    true
                } else {
                    false
                }
            });
        })
    }

    fn set_provider_visibilities(
        &mut self,
        value: HashMap<String, Vec<i32>>,
    ) -> Result<(), StorageError> {
        let value_str =
            serde_json::to_string(&value).map_err(|_| StorageError::SerializationFailure)?;
        self.set::<ProviderVisibilities>(value_str).map(|_| {
            let _ = self.watcher.send_if_modified(|preferences| {
                let is_same = preferences.provider_visibilities == value;
                if !is_same {
                    preferences.provider_visibilities = value;
                    return true;
                }
                false
            });
        })
    }
}

impl<S: PreferenceKeyValueStorage> PreferencesManagerImpl<S> {
    fn set<Q: IdentityManagerPreference>(
        &self,
        value: <Q as Key>::Value,
    ) -> Result<(), StorageError> {
        self.repository.set::<Q>(value)
    }

    fn get<Q: IdentityManagerPreference>(&self) -> <Q as Key>::Value {
        self.repository.get::<Q>().unwrap_or(Q::default_value())
    }

    fn get_all_preferences(storage: &impl PreferenceKeyValueStorage) -> IdentityManagerPreferences {
        let device_name = storage.get::<DeviceName>().unwrap_or(DeviceName::default_value());
        let device_type = storage.get::<DeviceType>().unwrap_or(DeviceType::default_value());
        let device_id =
            storage.get::<DeviceId>().unwrap_or(DeviceId::default_value()).to_be_bytes();
        let instance_type = storage.get::<InstanceType>().unwrap_or(InstanceType::default_value());
        let device_model_name =
            storage.get::<DeviceModelName>().unwrap_or(DeviceModelName::default_value());
        let device_manufacturer =
            storage.get::<DeviceManufacturer>().unwrap_or(DeviceManufacturer::default_value());
        let provider_visibilities_json =
            storage.get::<ProviderVisibilities>().unwrap_or(ProviderVisibilities::default_value());
        let provider_visibilities =
            serde_json::from_str(&provider_visibilities_json).unwrap_or_default();
        IdentityManagerPreferences {
            device_name,
            device_type,
            device_id,
            instance_type,
            device_model_name,
            device_manufacturer,
            provider_visibilities,
        }
    }

    /// Creates a new instance of the PreferencesManager with injected storage.
    pub(crate) fn new_with_storage(storage: S) -> PreferencesManagerImpl<S> {
        let sender = Sender::new(Self::get_all_preferences(&storage));
        PreferencesManagerImpl { watcher: sender, repository: storage }
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(
        watcher: Sender<IdentityManagerPreferences>,
        repository: S,
    ) -> PreferencesManagerImpl<S> {
        PreferencesManagerImpl { watcher, repository }
    }
}
