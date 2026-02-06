// Copyright (C) 2023 The Android Open Source Project
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

//! This crate exposes the Storage API in Rust, along with a platform agnostic API for retrieving the local directories of the host OS.

/// Database storage API definition.
pub mod database_storage;
/// File based storage API definition.
#[cfg(not(any(target_os = "macos", target_os = "ios")))]
mod file_based_storage;

/// User default based storage API.
#[cfg(all(
    any(target_os = "macos", target_os = "ios"),
    not(feature = "test-fake")
))]
mod user_default_storage;

// --- Fake Storage Implementation (Only included when test-fake feature is on) ---
#[cfg(feature = "test-fake")]
mod fake_storage;

#[cfg(feature = "test-fake")]
mod fake_registry {
    use crate::database_storage::fake::FakeDatabaseStorage;
    use crate::fake_storage::FakeStorage;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{LazyLock, Mutex};

    // Global map for FakeStorage instances, keyed by PathBuf.
    static FAKE_STORES: LazyLock<Mutex<HashMap<PathBuf, FakeStorage>>> =
        LazyLock::new(Mutex::default);
    static FAKE_DB_STORES: LazyLock<Mutex<HashMap<PathBuf, FakeDatabaseStorage>>> =
        LazyLock::new(Mutex::default);

    /// Returns a clone of the FakeStorage instance for the given path.
    /// Since FakeStorage uses Arc internally, cloning is cheap and shares the underlying map.
    pub fn get_fake_store(path: PathBuf) -> FakeStorage {
        let mut stores = FAKE_STORES.lock().unwrap();
        stores.entry(path).or_default().clone()
    }

    pub fn get_fake_db_store(path: PathBuf) -> FakeDatabaseStorage {
        let mut stores = FAKE_DB_STORES.lock().unwrap();
        stores.entry(path).or_default().clone()
    }
}
/// Value variant.
mod value_variant;
#[cfg(not(target_os = "android"))]
use directories::BaseDirs;
#[cfg(target_os = "android")]
use std::path::Path;
use std::path::PathBuf;
#[cfg(target_os = "android")]
use std::sync::OnceLock;
use thiserror::Error;

#[cfg(feature = "test-fake")]
pub use fake_storage::FakeStorage as PlatformStorageImpl;

#[cfg(not(any(target_os = "macos", target_os = "ios", feature = "test-fake")))]
pub use file_based_storage::{
    file_based_storage_impl::FileBasedStorageImpl as PlatformStorageImpl, FileBasedStorageError,
};
#[cfg(all(
    any(target_os = "macos", target_os = "ios"),
    not(feature = "test-fake")
))]
pub use user_default_storage::UserDefaultStorage as PlatformStorageImpl;
pub use value_variant::*;

#[cfg(feature = "test-fake")]
pub use database_storage::fake::FakeDatabaseStorage as PlatformDatabaseImpl;
#[cfg(not(feature = "test-fake"))]
pub use database_storage::sqlite::SqliteStorage as PlatformDatabaseImpl;

/// A platform agnostic representation of things that may have gone wrong while setting or getting a key.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Invalid home directory, unable to retrieve a usable path.
    #[error("No valid home directory path could be retrieved from the operating system.")]
    InvalidHomeDirectory,
    /// IO operation failures.
    #[error("Storage platform failed IO operation.")]
    IoError {
        #[from]
        /// Convert std::io::Error to StorageError
        source: std::io::Error,
    },
    /// Serialization and deserialization failures.
    #[error("Storage platform failed to serialize bytes.")]
    SerializationFailure,
    /// Wraps the `FileBasedStorageError` variant.
    #[error("File based storage platform had an error: `{0}`")]
    FileBasedStorageError(String),
    /// Wraps the `DatabaseStorageError` variant.
    #[error("Database storage platform had an error: `{0}`")]
    DatabaseStorageError(String),
}

/// A common trait for all preference stores to implement.
pub trait PreferenceKeyValueStorage: Send + Sync {
    /// Sets `key` to `value`. Returns nothing if succeeded, otherwise returns `StorageError`
    /// indicating the error. The old value for the key will not be returned if it already exists.
    fn set<K: Key>(&self, value: K::Value) -> Result<(), StorageError>;

    /// Retrieves the value associated with key, if present and accessible.
    fn get<K: Key>(&self) -> Option<K::Value>;
}

/// A common trait for database storage implementations.
pub trait DatabaseStorage: Send + Sync {
    /// Retrieves the value associated with key, if present and accessible.
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError>;

    /// Retrieves all key-value pairs where the key starts with the given prefix.
    fn get_by_prefix(&self, key_prefix: &str) -> Result<Vec<(String, Vec<u8>)>, StorageError>;

    /// Sets `key` to `value`. Returns nothing if succeeded, otherwise returns `StorageError`
    /// indicating the error. The old value for the key will not be returned if it already exists.
    fn insert(&self, key: &str, value: Vec<u8>) -> Result<(), StorageError>;

    /// Removes the value associated with the key and returns it.
    fn remove(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError>;
}

/// Platform agnostic API for retrieving the local app data directory of the host OS.
#[cfg(not(target_os = "android"))]
pub fn get_local_app_data_dir() -> Result<PathBuf, StorageError> {
    BaseDirs::new()
        .ok_or(StorageError::InvalidHomeDirectory)
        .map(|base_dirs| base_dirs.data_local_dir().to_path_buf())
}

#[cfg(target_os = "android")]
static LOCAL_APP_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Platform-agnostic API for retrieving the local app data directory of the host OS.
#[cfg(target_os = "android")]
pub fn get_local_app_data_dir() -> Result<PathBuf, StorageError> {
    LOCAL_APP_DATA_DIR
        .get()
        .cloned()
        .ok_or(StorageError::InvalidHomeDirectory)
}

/// Sets the local app data directory of the Android App.
#[cfg(target_os = "android")]
pub fn set_local_app_data_dir<P: AsRef<Path>>(path: P) {
    // SAFETY: an invalid app data dir is a fatal error.
    #[allow(clippy::expect_used)]
    LOCAL_APP_DATA_DIR
        .set(path.as_ref().to_path_buf())
        .expect("Invalid local app data dir");
}

/// Retrieves an instance of the store that supports a key-value abstraction.
pub fn get_store(path: PathBuf) -> impl PreferenceKeyValueStorage {
    #[cfg(feature = "test-fake")]
    {
        return fake_registry::get_fake_store(path);
    }

    #[cfg(not(feature = "test-fake"))]
    {
        PlatformStorageImpl::new(path)
    }
}

/// Retrieves an instance of the sqlite database storage.
pub fn get_sqlite_database(path: PathBuf) -> Result<impl DatabaseStorage, StorageError> {
    #[cfg(feature = "test-fake")]
    {
        return Ok(fake_registry::get_fake_db_store(path));
    }

    #[cfg(not(feature = "test-fake"))]
    {
        PlatformDatabaseImpl::new(path)
    }
}
