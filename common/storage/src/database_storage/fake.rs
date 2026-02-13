// Copyright 2025 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

use crate::{DatabaseStorage, StorageError};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// A fake, in-memory database storage implementation for testing.
#[derive(Clone, Default)]
pub struct FakeDatabaseStorage {
    map: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl FakeDatabaseStorage {
    /// Creates a new `FakeDatabaseStorage`. The `_path` is ignored, but included
    /// for API compatibility with other storage implementations.
    #[allow(dead_code)]
    pub fn new(_db_path: impl AsRef<Path>) -> Result<Self, StorageError> {
        Ok(Self::default())
    }
}

impl DatabaseStorage for FakeDatabaseStorage {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let map = self.map.lock().unwrap();
        Ok(map.get(key).cloned())
    }

    fn get_by_prefix(&self, key_prefix: &str) -> Result<Vec<(String, Vec<u8>)>, StorageError> {
        let map = self.map.lock().unwrap();
        let results = map
            .iter()
            .filter(|(k, _)| k.starts_with(key_prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Ok(results)
    }

    fn insert(&self, key: &str, value: Vec<u8>) -> Result<(), StorageError> {
        let mut map = self.map.lock().unwrap();
        let _ = map.insert(key.to_string(), value);
        Ok(())
    }

    fn remove(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let mut map = self.map.lock().unwrap();
        Ok(map.remove(key))
    }
}
