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

#![allow(dead_code, unused_imports)]

#[cfg(feature = "encrypted_identity")]
pub(crate) mod credentials_database;
pub(crate) mod preferences_manager_impl;

use identity_common::definitions::ProviderName;
use log::info;
use std::{
    collections::{HashMap, HashSet},
    fs,
    sync::Arc,
};
use storage::{Key, PlatformStorageImpl, PreferenceKeyValueStorage, StorageError};
use tokio::sync::{watch::Receiver, Mutex};

pub(crate) const IDENTITY_PREFERENCES_FILE_PATH: &str = "Google/Nearby/identity/preferences.json";
/// Length of stable device ID in bytes.
pub const DEVICE_ID_LENGTH: usize = 16;

mod values {
    pub trait Sealed {}
}

/// Marker trait that shows this preference is a part of Nearby identity manager preferences.
pub(crate) trait IdentityManagerPreference: values::Sealed + Sized + Key {
    fn default_value() -> <Self as Key>::Value;
}

/// Device name preference.
pub struct DeviceName {}
impl values::Sealed for DeviceName {}
impl IdentityManagerPreference for DeviceName {
    fn default_value() -> <Self as Key>::Value {
        device_info::device_name()
    }
}
impl Key for DeviceName {
    const KEY: &'static str = "device_name";
    type Value = String;
}

/// Device type preference.
pub struct DeviceType {}
impl values::Sealed for DeviceType {}
impl IdentityManagerPreference for DeviceType {
    fn default_value() -> <Self as Key>::Value {
        device_info::device_type() as u8
    }
}
impl Key for DeviceType {
    const KEY: &'static str = "device_type";
    type Value = u8;
}

/// Device ID preference.
pub struct DeviceId {}
impl values::Sealed for DeviceId {}
impl IdentityManagerPreference for DeviceId {
    fn default_value() -> <Self as Key>::Value {
        0u128
    }
}
impl Key for DeviceId {
    const KEY: &'static str = "device_id";
    type Value = u128;
}

/// Instance type preference.
pub struct InstanceType {}
impl values::Sealed for InstanceType {}
impl IdentityManagerPreference for InstanceType {
    fn default_value() -> <Self as Key>::Value {
        0u8
    }
}
impl Key for InstanceType {
    const KEY: &'static str = "instance_type";
    type Value = u8;
}

/// Device model name preference.
pub struct DeviceModelName {}
impl values::Sealed for DeviceModelName {}
impl IdentityManagerPreference for DeviceModelName {
    fn default_value() -> <Self as Key>::Value {
        String::new()
    }
}
impl Key for DeviceModelName {
    const KEY: &'static str = "device_model_name";
    type Value = String;
}

/// Device manufacturer preference.
pub struct DeviceManufacturer {}
impl values::Sealed for DeviceManufacturer {}
impl IdentityManagerPreference for DeviceManufacturer {
    fn default_value() -> <Self as Key>::Value {
        String::new()
    }
}
impl Key for DeviceManufacturer {
    const KEY: &'static str = "device_manufacturer";
    type Value = String;
}

/// Provider visibilities preference.
pub struct ProviderVisibilities {}
impl values::Sealed for ProviderVisibilities {}
impl IdentityManagerPreference for ProviderVisibilities {
    fn default_value() -> <Self as Key>::Value {
        "{}".to_string()
    }
}
impl Key for ProviderVisibilities {
    const KEY: &'static str = "provider_visibilities";
    type Value = String;
}

/// Encapsulates all the values for possible identity manager preferences.
#[derive(Clone, Debug, PartialEq)]
pub struct IdentityManagerPreferences {
    /// The name of the device.
    pub device_name: <DeviceName as Key>::Value,
    /// The type of the device.
    pub device_type: <DeviceType as Key>::Value,
    /// The unique ID of the device.
    pub device_id: [u8; DEVICE_ID_LENGTH],
    /// The instance type of the device.
    pub instance_type: <InstanceType as Key>::Value,
    /// The model name of the device.
    pub device_model_name: <DeviceModelName as Key>::Value,
    /// The manufacturer of the device.
    pub device_manufacturer: <DeviceManufacturer as Key>::Value,
    /// The visibilities configured for each provider.
    pub provider_visibilities: HashMap<String, Vec<i32>>,
}

impl Default for IdentityManagerPreferences {
    fn default() -> Self {
        IdentityManagerPreferences {
            device_name: DeviceName::default_value(),
            device_type: DeviceType::default_value(),
            device_id: [0; DEVICE_ID_LENGTH],
            instance_type: InstanceType::default_value(),
            device_model_name: DeviceModelName::default_value(),
            device_manufacturer: DeviceManufacturer::default_value(),
            provider_visibilities: HashMap::new(),
        }
    }
}

/// Manages the preferences for the Identity Manager.
pub(crate) trait PreferencesManager: Send + Sync {
    /// Subscribes to changes in the preferences.
    fn subscribe(&self) -> Receiver<IdentityManagerPreferences>;
    /// Sets the current device name.
    fn set_device_name(&mut self, value: <DeviceName as Key>::Value) -> Result<(), StorageError>;
    /// Sets the current device type.
    fn set_device_type(&mut self, value: <DeviceType as Key>::Value) -> Result<(), StorageError>;
    /// Sets the current device ID.
    fn set_device_id(&mut self, value: [u8; DEVICE_ID_LENGTH]) -> Result<(), StorageError>;
    /// Sets the current instance type.
    fn set_instance_type(
        &mut self,
        value: <InstanceType as Key>::Value,
    ) -> Result<(), StorageError>;
    /// Sets the current device model name.
    fn set_device_model_name(
        &mut self,
        value: <DeviceModelName as Key>::Value,
    ) -> Result<(), StorageError>;
    /// Sets the current device manufacturer.
    fn set_device_manufacturer(
        &mut self,
        value: <DeviceManufacturer as Key>::Value,
    ) -> Result<(), StorageError>;
    /// Sets the provider visibilities.
    fn set_provider_visibilities(
        &mut self,
        value: HashMap<String, Vec<i32>>,
    ) -> Result<(), StorageError>;
}

pub(crate) type PreferencesManagerResult =
    (Arc<Mutex<dyn PreferencesManager>>, Receiver<IdentityManagerPreferences>);

/// Creates a new instance of the PreferencesManager with given storage repository.
pub(crate) fn create_preferences_manager(
    repo: impl PreferenceKeyValueStorage + 'static,
) -> PreferencesManagerResult {
    let pm_impl = preferences_manager_impl::PreferencesManagerImpl::new_with_storage(repo);
    let prefs_reader = pm_impl.subscribe();
    (Arc::new(Mutex::new(pm_impl)), prefs_reader)
}

pub(crate) fn create_preferences_storage() -> Result<impl PreferenceKeyValueStorage, StorageError> {
    let path = storage::get_local_app_data_dir()?.join(IDENTITY_PREFERENCES_FILE_PATH);
    // SAFETY: parent folder always exists.
    #[allow(clippy::expect_used)]
    fs::create_dir_all(path.parent().expect("No parent directory"))
        .expect("Failed to create preferences storage");
    Ok(storage::get_store(path))
}
