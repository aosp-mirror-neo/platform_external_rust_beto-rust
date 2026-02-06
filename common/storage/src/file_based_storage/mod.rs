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

use thiserror::Error;

// File based storage API implementation.
pub(crate) mod file_based_storage_impl;

/// Errors exposed by the FileBasedStorage API.
#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum FileBasedStorageError {
    /// Save failures.
    #[error("FileBasedStorage platform failed to save file.")]
    SaveFailure,
    /// Load failures.
    #[error("FileBasedStorage platform failed to load file.")]
    LoadFailure,
    /// Serialization and deserialization failures.
    #[error("FileBasedStorage failed to serialize bytes.")]
    SerializationFailure {
        #[from]
        /// Convert serde_json::Error to FileBasedStorageError
        source: serde_json::Error,
    },
    /// IO operation failures.
    #[error("FileBasedStorage platform failed IO operation.")]
    IoError {
        #[from]
        /// Convert std::io::Error to FileBasedStorageError
        source: std::io::Error,
    },
    /// No valid home directory failure.
    #[error("No valid home directory path could be retrieved from the operating system.")]
    InvalidHomeDirectory,
    /// Invalid file path.
    #[error("Invalid file path")]
    InvalidFilePath,
    /// File does not exist.
    #[error("File does not exist")]
    FileDoesNotExist,
}
