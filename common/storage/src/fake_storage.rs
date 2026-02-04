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
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

use crate::{Key, PreferenceKeyValueStorage, StorageError};
use std::any::Any;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// A fake, in-memory storage implementation for testing.
#[derive(Clone, Default)]
#[allow(dead_code)]
pub struct FakeStorage {
    map: Arc<Mutex<HashMap<String, Box<dyn Any + Send + Sync>>>>,
}

impl PreferenceKeyValueStorage for FakeStorage {
    fn set<K: Key>(&self, value: K::Value) -> Result<(), StorageError> {
        let mut map = self.map.lock().unwrap();
        let _ = map.insert(K::KEY.to_string(), Box::new(value));
        Ok(())
    }

    fn get<K: Key>(&self) -> Option<K::Value> {
        let map = self.map.lock().unwrap();
        map.get(K::KEY)
            .and_then(|v| v.downcast_ref::<K::Value>())
            .cloned()
    }
}

impl FakeStorage {
    /// Creates a new `FakeStorage`. The `_path` is ignored, but included
    /// for API compatibility with other storage implementations.
    #[allow(dead_code)]
    pub fn new(_path: PathBuf) -> Self {
        Self::default()
    }
}
