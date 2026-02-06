// Copyright (C) 2024 The Android Open Source Project
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Storage implementation tests.

// If this ever fails we want to know during local testing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{path::PathBuf, sync::Mutex};
use storage::{Key, PreferenceKeyValueStorage};

fn with_global_lock<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    static MUTEX: Mutex<()> = Mutex::new(());
    let _guard = MUTEX.lock().unwrap();
    // Any assertion failure here will poison the mutex and not allow it to be locked again, failing to lock.
    f()
}

#[allow(unexpected_cfgs)]
fn create_storage() -> impl PreferenceKeyValueStorage + Sized {
    let file_path = "test/file/path/test_file_name.json";
    #[cfg(not(blaze))]
    let out_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    #[cfg(blaze)]
    let out_dir: PathBuf = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap());
    let full_path = out_dir.join(file_path);
    storage::get_store(full_path)
}

struct DeviceName {}

impl Key for DeviceName {
    const KEY: &str = "device_name";
    type Value = String;
}

#[repr(u32)]
enum Visibility {
    Hidden = 0,
}

impl Key for Visibility {
    const KEY: &str = "visibility";
    type Value = u32;
}

#[test]
fn test_set_and_get_string() {
    with_global_lock(|| {
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp_millis()
            .filter_level(log::LevelFilter::Debug)
            .try_init();
        let storage = create_storage();
        assert!(storage.set::<DeviceName>("Unknown".to_string()).is_ok());
        let stored_name = storage.get::<DeviceName>();
        assert_eq!(stored_name, Some("Unknown".to_string()))
    });
}

#[test]
fn test_set_and_get_u32() {
    with_global_lock(|| {
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp_millis()
            .filter_level(log::LevelFilter::Debug)
            .try_init();
        let storage = create_storage();
        assert!(storage.set::<Visibility>(Visibility::Hidden as u32).is_ok());
        let stored_vis = storage.get::<Visibility>();
        assert_eq!(stored_vis, Some(Visibility::Hidden as u32))
    })
}

#[test]
fn test_set_and_get_file_exists() {
    with_global_lock(|| {
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp_millis()
            .filter_level(log::LevelFilter::Debug)
            .try_init();
        let storage = create_storage();
        assert!(storage.set::<DeviceName>("Unknown".to_string()).is_ok());
        let stored_name = storage.get::<DeviceName>();
        assert_eq!(stored_name, Some("Unknown".to_string()));
        assert!(storage.set::<DeviceName>("test pc".into()).is_ok());
        assert_eq!(storage.get::<DeviceName>(), Some("test pc".into()));
    })
}
