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

use crate::{DatabaseStorage, StorageError};
use rusqlite::{Connection, OptionalExtension, ToSql};
use std::path::Path;
use std::sync::Mutex;

/// A Sqlite-backed implementation of the `DatabaseStorage` trait.
/// It is thread-safe and stores data as raw byte blobs.
pub struct SqliteStorage {
    connection: Mutex<Connection>,
}

impl SqliteStorage {
    /// Opens a new or existing SQLite database file.
    ///
    /// This will create the database file and the necessary tables if they don't exist.
    pub fn new(db_path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let connection = Connection::open(db_path)
            .map_err(|e| StorageError::DatabaseStorageError(e.to_string()))?;
        let _ = connection
            .execute(
                "CREATE TABLE IF NOT EXISTS data_store (
                key TEXT PRIMARY KEY,
                value BLOB NOT NULL
            )",
                [],
            )
            .map_err(|e| StorageError::DatabaseStorageError(e.to_string()))?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }
}

impl DatabaseStorage for SqliteStorage {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| StorageError::DatabaseStorageError("Mutex poisoned".to_string()))?;
        let mut stmt = conn
            .prepare("SELECT value FROM data_store WHERE key = ?1")
            .map_err(|e| StorageError::DatabaseStorageError(e.to_string()))?;

        match stmt.query_row([key], |row| row.get(0)).optional() {
            Ok(maybe_value) => Ok(maybe_value),
            Err(e) => Err(StorageError::DatabaseStorageError(e.to_string())),
        }
    }

    fn get_by_prefix(&self, key_prefix: &str) -> Result<Vec<(String, Vec<u8>)>, StorageError> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| StorageError::DatabaseStorageError("Mutex poisoned".to_string()))?;
        let mut stmt = conn
            .prepare("SELECT key, value FROM data_store WHERE key LIKE ?1")
            .map_err(|e| StorageError::DatabaseStorageError(e.to_string()))?;

        let pattern = format!("{key_prefix}%");
        let rows = stmt
            .query_map([pattern], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| StorageError::DatabaseStorageError(e.to_string()))?;

        let mut results = Vec::new();
        for row_result in rows {
            results
                .push(row_result.map_err(|e| StorageError::DatabaseStorageError(e.to_string()))?);
        }
        Ok(results)
    }

    fn insert(&self, key: &str, value: Vec<u8>) -> Result<(), StorageError> {
        let conn = self
            .connection
            .lock()
            .map_err(|_| StorageError::DatabaseStorageError("Mutex poisoned".to_string()))?;
        let params: &[&dyn ToSql] = &[&key, &value];
        let _ = conn
            .execute(
                "INSERT OR REPLACE INTO data_store (key, value) VALUES (?1, ?2)",
                params,
            )
            .map_err(|e| StorageError::DatabaseStorageError(e.to_string()))?;
        Ok(())
    }

    fn remove(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let mut conn = self
            .connection
            .lock()
            .map_err(|_| StorageError::DatabaseStorageError("Mutex poisoned".to_string()))?;

        let tx = conn
            .transaction()
            .map_err(|e| StorageError::DatabaseStorageError(e.to_string()))?;

        let maybe_value: Option<Vec<u8>> = tx
            .query_row(
                "SELECT value FROM data_store WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| StorageError::DatabaseStorageError(e.to_string()))?;

        if maybe_value.is_some() {
            let _ = tx
                .execute("DELETE FROM data_store WHERE key = ?1", [key])
                .map_err(|e| StorageError::DatabaseStorageError(e.to_string()))?;
        }

        tx.commit()
            .map_err(|e| StorageError::DatabaseStorageError(e.to_string()))?;

        Ok(maybe_value)
    }
}
