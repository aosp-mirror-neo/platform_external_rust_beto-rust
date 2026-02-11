// Copyright 2023 Google LLC
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
#![allow(clippy::expect_used)]

use crate::file_based_storage::FileBasedStorageError;
use crate::{
    value_variant::values::Token, Key, PreferenceKeyValueStorage, StorageError, ValueType,
};
use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// File based storage implementation.
pub struct FileBasedStorageImpl {
    file_path: PathBuf,
    mutex: Mutex<()>,
}

impl PreferenceKeyValueStorage for FileBasedStorageImpl {
    fn set<K: Key>(&self, value: K::Value) -> Result<(), StorageError> {
        let _lock = self
            .mutex
            .lock()
            .expect("We should be able to acquire the FileBasedStorage lock.");

        let mut map: HashMap<String, serde_json::Value> = if Self::file_exists(&self.file_path) {
            let json_bytes = self
                .get_bytes_locked()
                .map_err(|e| StorageError::FileBasedStorageError(e.to_string()))?;
            serde_json::from_slice(&json_bytes).map_err(|_| StorageError::SerializationFailure)
        } else {
            Ok(HashMap::new())
        }?;
        let final_value = value.to_value(Token);
        if let Some(existing_value) = map.get(K::KEY) {
            if existing_value == &final_value {
                // The value already exists, don't bother doing anything else.
                return Ok(());
            }
        }
        let _ = map.insert(K::KEY.to_string(), final_value);
        let new_bytes = serde_json::to_vec(&map).map_err(|_| StorageError::SerializationFailure)?;
        self.set_bytes_locked(new_bytes)
            .map_err(|e| StorageError::FileBasedStorageError(e.to_string()))
    }

    fn get<K: Key>(&self) -> Option<K::Value> {
        let _lock = self
            .mutex
            .lock()
            .expect("We should be able to acquire the FileBasedStorage lock.");

        let json_bytes = self.get_bytes_locked().ok()?;
        let map: HashMap<String, serde_json::Value> = serde_json::from_slice(&json_bytes).ok()?;
        map.get(K::KEY)
            .and_then(|v| K::Value::try_from_value(v.clone(), Token).ok())
    }
}

#[allow(unused)]
impl FileBasedStorageImpl {
    /// Creates a new file-based storage implementation.
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            mutex: Mutex::new(()),
        }
    }

    fn set_bytes_locked(&self, data: Vec<u8>) -> Result<(), FileBasedStorageError> {
        let parent_dir = PathBuf::from(
            self.file_path
                .parent()
                .ok_or(FileBasedStorageError::InvalidFilePath)?,
        );
        if !Self::file_exists(&self.file_path) {
            let result = Self::create_directories(&parent_dir);
            {
                if let Err(err) = result {
                    if err.kind() == ErrorKind::AlreadyExists {
                        // The directory already exists.
                        return Ok(());
                    }
                    log::error!("Failed to create data path with error: {err:?}.");
                    Err(FileBasedStorageError::from(err))
                } else {
                    Ok(())
                }
            }?;
        }

        // Create {stem}_backup.{extension} file:
        let mut backup_path = parent_dir;
        let mut file_stem = self
            .file_path
            .file_stem()
            .ok_or(FileBasedStorageError::InvalidFilePath)?
            .to_string_lossy()
            .to_string();
        file_stem.push_str("_backup");
        backup_path.push(file_stem);
        let _ = backup_path.set_extension(
            self.file_path
                .extension()
                .ok_or(FileBasedStorageError::InvalidFilePath)?,
        );
        if Self::file_exists(&self.file_path) {
            log::info!("Making backup of data file.");
            Self::rename(&self.file_path, &backup_path)?;
        }

        fs::write(&self.file_path, &data)
            .map(|()| {
                self.attempt_load().map(|_| Ok(())).map_err(|err| {
                    log::error!("Data saved to disk in corrupted state: {err}");
                    err
                })?
            })
            .map_err(|err| {
                log::error!("Failed to save data file: {err}");
                err
            })?
    }

    fn get_bytes_locked(&self) -> Result<Vec<u8>, FileBasedStorageError> {
        self.attempt_load().map_err(|err| {
            log::error!("Failed to load data: {err}");
            err
        })
    }

    fn attempt_load(&self) -> Result<Vec<u8>, FileBasedStorageError> {
        let parent_dir = PathBuf::from(
            self.file_path
                .parent()
                .ok_or(FileBasedStorageError::InvalidFilePath)?,
        );
        log::debug!(
            "Parent dir: {parent_dir:?}, file_path: {:?}",
            &self.file_path
        );
        if !Self::directory_exists(&parent_dir) || !Self::file_exists(&self.file_path) {
            log::warn!("Could not load data file, trying backup.");
            return self.restore_from_backup();
        }

        fs::read(&self.file_path).map_err(FileBasedStorageError::from)
    }

    fn restore_from_backup(&self) -> Result<Vec<u8>, FileBasedStorageError> {
        let parent_dir = PathBuf::from(
            self.file_path
                .parent()
                .ok_or(FileBasedStorageError::InvalidFilePath)?,
        );
        let file_name = self
            .file_path
            .file_name()
            .ok_or(FileBasedStorageError::InvalidFilePath)?;
        let backup_file_name = file_name.to_string_lossy().replace(".json", "_backup.json");
        let backup_path = parent_dir.join(backup_file_name);

        if !Self::file_exists(&backup_path) {
            log::warn!("Backup requested but no backup data file found.");
            return Err(FileBasedStorageError::FileDoesNotExist);
        }

        Self::rename(&backup_path, &self.file_path)?;

        log::info!("Attempting to read from backup data.");
        fs::read(&self.file_path).map_err(FileBasedStorageError::from)
    }

    fn file_exists(path: &Path) -> bool {
        path.metadata().map(|m| m.is_file()).unwrap_or(false)
    }

    fn rename(old_path: &PathBuf, new_path: &PathBuf) -> Result<(), std::io::Error> {
        log::debug!("old_path: {old_path:?}");
        log::debug!("new_path: {new_path:?}");
        std::fs::rename(old_path, new_path).map_err(|err| {
            log::error!("Failed to rename data backup file: {err}");
            err
        })
    }

    fn directory_exists(path: &Path) -> bool {
        path.metadata().map(|m| m.is_dir()).unwrap_or(false)
    }

    fn create_directories(path: &PathBuf) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(path)
    }
}
