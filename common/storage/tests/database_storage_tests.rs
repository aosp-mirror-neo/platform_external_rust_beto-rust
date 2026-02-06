// Copyright (C) 2025 The Android Open Source Project
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

//! Sqlite storage implementation tests.

#![allow(clippy::unwrap_used, unexpected_cfgs)]

use std::path::PathBuf;
use storage::database_storage::sqlite::SqliteStorage;
use storage::DatabaseStorage;

fn create_sqlite_storage(db_name: &str) -> SqliteStorage {
    #[cfg(not(blaze))]
    let out_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    #[cfg(blaze)]
    let out_dir: PathBuf = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap());
    let db_path = out_dir.join(db_name);
    // In case a previous test run failed.
    let _ = std::fs::remove_file(&db_path);
    SqliteStorage::new(&db_path).unwrap()
}

#[test]
fn test_new_db_creates_file() {
    #[cfg(not(blaze))]
    let out_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    #[cfg(blaze)]
    let out_dir: PathBuf = PathBuf::from(std::env::var("TEST_TMPDIR").unwrap());
    let db_path = out_dir.join("test_new.db");
    let _ = std::fs::remove_file(&db_path);

    assert!(!db_path.exists());
    let _storage = SqliteStorage::new(&db_path).unwrap();
    assert!(db_path.exists());
    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn test_insert_and_get() {
    let storage = create_sqlite_storage("test_insert_get.db");
    let key = "test_key";
    let value = b"test_value".to_vec();

    storage.insert(key, value.clone()).unwrap();

    let retrieved_value = storage.get(key).unwrap();
    assert_eq!(retrieved_value, Some(value));

    let non_existent = storage.get("non_existent_key").unwrap();
    assert_eq!(non_existent, None);
}

#[test]
fn test_update_value() {
    let storage = create_sqlite_storage("test_update.db");
    let key = "test_key";
    let value1 = b"value1".to_vec();
    let value2 = b"value2".to_vec();

    storage.insert(key, value1).unwrap();
    storage.insert(key, value2.clone()).unwrap();

    let retrieved_value = storage.get(key).unwrap();
    assert_eq!(retrieved_value, Some(value2));
}

#[test]
fn test_remove() {
    let storage = create_sqlite_storage("test_remove.db");
    let key = "test_key";
    let value = b"test_value".to_vec();

    storage.insert(key, value.clone()).unwrap();

    let removed_value = storage.remove(key).unwrap();
    assert_eq!(removed_value, Some(value));

    let retrieved_value = storage.get(key).unwrap();
    assert_eq!(retrieved_value, None);

    let non_existent_removed = storage.remove("non_existent_key").unwrap();
    assert_eq!(non_existent_removed, None);
}

#[test]
fn test_get_by_prefix() {
    let storage = create_sqlite_storage("test_prefix.db");
    let key1 = "prefix_a_1";
    let value1 = b"value1".to_vec();
    let key2 = "prefix_a_2";
    let value2 = b"value2".to_vec();
    let key3 = "prefix_b_1";
    let value3 = b"value3".to_vec();

    storage.insert(key1, value1.clone()).unwrap();
    storage.insert(key2, value2.clone()).unwrap();
    storage.insert(key3, value3.clone()).unwrap();

    let mut results = storage.get_by_prefix("prefix_a").unwrap();
    results.sort_by(|a, b| a.0.cmp(&b.0));

    assert_eq!(results.len(), 2);
    assert_eq!(results.first().unwrap(), &(key1.to_string(), value1));
    assert_eq!(results.get(1).unwrap(), &(key2.to_string(), value2));

    let empty_results = storage.get_by_prefix("prefix_c").unwrap();
    assert!(empty_results.is_empty());
}

#[test]
fn test_get_by_prefix_empty_prefix() {
    let storage = create_sqlite_storage("test_empty_prefix.db");
    let key1 = "key1";
    let value1 = b"value1".to_vec();
    let key2 = "key2";
    let value2 = b"value2".to_vec();

    storage.insert(key1, value1.clone()).unwrap();
    storage.insert(key2, value2.clone()).unwrap();

    let mut results = storage.get_by_prefix("").unwrap();
    results.sort_by(|a, b| a.0.cmp(&b.0));

    assert_eq!(results.len(), 2);
    assert_eq!(results.first().unwrap(), &(key1.to_string(), value1));
    assert_eq!(results.get(1).unwrap(), &(key2.to_string(), value2));
}
