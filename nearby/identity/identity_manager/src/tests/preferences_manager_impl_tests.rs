#![allow(clippy::expect_used)]
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
    preferences_manager_impl::PreferencesManagerImpl, DeviceId, DeviceManufacturer,
    DeviceModelName, DeviceName, DeviceType, IdentityManagerPreference, IdentityManagerPreferences,
    InstanceType, PreferencesManager,
};
use serial_test::serial;
use std::{collections::HashMap, path::PathBuf};
use storage::PreferenceKeyValueStorage;
use tempfile::tempdir;

fn new_for_test(path: PathBuf) -> PreferencesManagerImpl<impl PreferenceKeyValueStorage> {
    let repository = storage::get_store(path);

    // Replicate the logic of `get_all_preferences` to initialize the watcher
    // correctly for the tests. This is a workaround for the type inference
    // issue with the `impl Trait` return type of `storage::get_store`.
    let device_name = repository.get::<DeviceName>().unwrap_or(DeviceName::default_value());
    let device_type = repository.get::<DeviceType>().unwrap_or(DeviceType::default_value());
    let device_id = repository.get::<DeviceId>().unwrap_or(DeviceId::default_value()).to_be_bytes();
    let instance_type = repository.get::<InstanceType>().unwrap_or(InstanceType::default_value());
    let device_model_name =
        repository.get::<DeviceModelName>().unwrap_or(DeviceModelName::default_value());
    let device_manufacturer =
        repository.get::<DeviceManufacturer>().unwrap_or(DeviceManufacturer::default_value());
    let initial_prefs = IdentityManagerPreferences {
        device_name,
        device_type,
        device_id,
        instance_type,
        device_model_name,
        device_manufacturer,
        provider_visibilities: HashMap::new(),
    };
    let (sender, _) = tokio::sync::watch::channel(initial_prefs);

    PreferencesManagerImpl::new_for_test(sender, repository)
}

#[test]
#[serial]
fn test_device_name_default() {
    let dir = tempdir().expect("test expectation failed");
    let file_path = dir.path().join("test.json");
    let manager = new_for_test(file_path);
    assert_eq!(manager.subscribe().borrow().device_name, DeviceName::default_value());
}

#[test]
#[serial]
fn test_set_and_get_device_name() {
    let dir = tempdir().expect("test expectation failed");
    let file_path = dir.path().join("test.json");
    let mut manager = new_for_test(file_path);
    let new_name = "My Test Device".to_string();

    assert!(manager.set_device_name(new_name.clone()).is_ok());
    assert_eq!(manager.subscribe().borrow().device_name, new_name);
}

#[tokio::test]
#[serial]
async fn test_device_name_watcher_notified() {
    let dir = tempdir().expect("test expectation failed");
    let file_path = dir.path().join("test.json");
    let mut manager = new_for_test(file_path);
    let mut receiver = manager.subscribe();

    // Initial value
    assert_eq!(receiver.borrow().device_name, DeviceName::default_value());

    let new_name = "New Device Name".to_string();
    manager.set_device_name(new_name.clone()).expect("test expectation failed");

    // Wait for the watcher to be notified
    assert!(receiver.changed().await.is_ok());
    assert_eq!(receiver.borrow().device_name, new_name);
}

#[tokio::test]
#[serial]
async fn test_device_name_watcher_not_notified_if_same_value() {
    let dir = tempdir().expect("test expectation failed");
    let file_path = dir.path().join("test.json");
    let mut manager = new_for_test(file_path);
    let mut receiver = manager.subscribe();

    let initial_name = manager.subscribe().borrow().device_name.clone();
    manager.set_device_name(initial_name.clone()).expect("test expectation failed");

    // Should not be notified
    assert!(tokio::time::timeout(std::time::Duration::from_millis(100), receiver.changed())
        .await
        .is_err());

    // The value should still be the same.
    assert_eq!(receiver.borrow().device_name, initial_name);
}

#[test]
#[serial]
fn test_device_id_default() {
    let dir = tempdir().expect("test expectation failed");
    let file_path = dir.path().join("test.json");
    let manager = new_for_test(file_path);
    assert_eq!(manager.subscribe().borrow().device_id, [0; 16]);
}

#[test]
#[serial]
fn test_set_and_get_device_id() {
    let dir = tempdir().expect("test expectation failed");
    let file_path = dir.path().join("test.json");
    let mut manager = new_for_test(file_path);
    let new_id = [1u8; 16];

    assert!(manager.set_device_id(new_id).is_ok());
    assert_eq!(manager.subscribe().borrow().device_id, new_id);
}

#[tokio::test]
#[serial]
async fn test_device_id_watcher_notified() {
    let dir = tempdir().expect("test expectation failed");
    let file_path = dir.path().join("test.json");
    let mut manager = new_for_test(file_path);
    let mut receiver = manager.subscribe();

    assert_eq!(receiver.borrow().device_id, [0; 16]);

    let new_id = [2u8; 16];
    manager.set_device_id(new_id).expect("test expectation failed");

    assert!(receiver.changed().await.is_ok());
    assert_eq!(receiver.borrow().device_id, new_id);
}

#[test]
#[serial]
fn test_instance_type_default() {
    let dir = tempdir().expect("test expectation failed");
    let file_path = dir.path().join("test.json");
    let manager = new_for_test(file_path);
    assert_eq!(manager.subscribe().borrow().instance_type, InstanceType::default_value());
}

#[test]
#[serial]
fn test_set_and_get_instance_type() {
    let dir = tempdir().expect("test expectation failed");
    let file_path = dir.path().join("test.json");
    let mut manager = new_for_test(file_path);
    let new_type = 1;

    assert!(manager.set_instance_type(new_type).is_ok());
    assert_eq!(manager.subscribe().borrow().instance_type, new_type);
}

#[tokio::test]
#[serial]
async fn test_instance_type_watcher_notified() {
    let dir = tempdir().expect("test expectation failed");
    let file_path = dir.path().join("test.json");
    let mut manager = new_for_test(file_path);
    let mut receiver = manager.subscribe();

    assert_eq!(receiver.borrow().instance_type, InstanceType::default_value());

    let new_type = 2;
    manager.set_instance_type(new_type).expect("test expectation failed");

    assert!(receiver.changed().await.is_ok());
    assert_eq!(receiver.borrow().instance_type, new_type);
}

#[test]
#[serial]
fn test_device_model_name_default() {
    let dir = tempdir().expect("test expectation failed");
    let file_path = dir.path().join("test.json");
    let manager = new_for_test(file_path);
    assert_eq!(manager.subscribe().borrow().device_model_name, DeviceModelName::default_value());
}

#[test]
#[serial]
fn test_set_and_get_device_model_name() {
    let dir = tempdir().expect("test expectation failed");
    let file_path = dir.path().join("test.json");
    let mut manager = new_for_test(file_path);
    let new_name = "Pixel 8".to_string();

    assert!(manager.set_device_model_name(new_name.clone()).is_ok());
    assert_eq!(manager.subscribe().borrow().device_model_name, new_name);
}

#[tokio::test]
#[serial]
async fn test_device_model_name_watcher_notified() {
    let dir = tempdir().expect("test expectation failed");
    let file_path = dir.path().join("test.json");
    let mut manager = new_for_test(file_path);
    let mut receiver = manager.subscribe();

    assert_eq!(receiver.borrow().device_model_name, DeviceModelName::default_value());

    let new_name = "Pixel 8 Pro".to_string();
    manager.set_device_model_name(new_name.clone()).expect("test expectation failed");

    assert!(receiver.changed().await.is_ok());
    assert_eq!(receiver.borrow().device_model_name, new_name);
}

#[test]
#[serial]
fn test_device_manufacturer_default() {
    let dir = tempdir().expect("test expectation failed");
    let file_path = dir.path().join("test.json");
    let manager = new_for_test(file_path);
    assert_eq!(
        manager.subscribe().borrow().device_manufacturer,
        DeviceManufacturer::default_value()
    );
}

#[test]
#[serial]
fn test_set_and_get_device_manufacturer() {
    let dir = tempdir().expect("test expectation failed");
    let file_path = dir.path().join("test.json");
    let mut manager = new_for_test(file_path);
    let new_manufacturer = "Google".to_string();

    assert!(manager.set_device_manufacturer(new_manufacturer.clone()).is_ok());
    assert_eq!(manager.subscribe().borrow().device_manufacturer, new_manufacturer);
}

#[tokio::test]
#[serial]
async fn test_device_manufacturer_watcher_notified() {
    let dir = tempdir().expect("test expectation failed");
    let file_path = dir.path().join("test.json");
    let mut manager = new_for_test(file_path);
    let mut receiver = manager.subscribe();

    assert_eq!(receiver.borrow().device_manufacturer, DeviceManufacturer::default_value());

    let new_manufacturer = "Goog".to_string();
    manager.set_device_manufacturer(new_manufacturer.clone()).expect("test expectation failed");

    assert!(receiver.changed().await.is_ok());
    assert_eq!(receiver.borrow().device_manufacturer, new_manufacturer);
}
