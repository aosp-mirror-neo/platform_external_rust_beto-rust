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

// Allow unsafe code here, as everything `objc2` exports is unsafe.
#![allow(unsafe_code)]

use crate::{
    value_variant::values::Token, Key, PreferenceKeyValueStorage, StorageError, ValueType,
};
use objc2_foundation::{NSObject, NSString, NSUserDefaults};
use std::path::PathBuf;

/// An implementation of `Storage` using the `NSUserDefaults` library on macOS and iOS.
pub struct UserDefaultStorage {}

impl PreferenceKeyValueStorage for UserDefaultStorage {
    // SAFETY: Only using generated functions on NSUserDefaults,and `Retained` is ref-counted,
    // so it gets dropped at the end of this function.
    fn set<K: Key>(&self, value: K::Value) -> Result<(), StorageError> {
        let storage = NSUserDefaults::standardUserDefaults();
        let key = NSString::from_str(K::KEY);
        unsafe { storage.setObject_forKey(Some(&value.to_nsobject(Token)), &key) };
        Ok(())
    }

    // SAFETY: Only using generated functions on NSUserDefaults,and `Retained` is ref-counted,
    // so it gets dropped at the end of this function.
    fn get<K: Key>(&self) -> Option<K::Value> {
        let storage = NSUserDefaults::standardUserDefaults();
        let key = NSString::from_str(K::KEY);
        let object = storage.objectForKey(&key)?;
        let nsobject = object.downcast_ref::<NSObject>()?;

        K::Value::try_from_nsobject(nsobject, Token).ok()
    }
}

impl UserDefaultStorage {
    /// Creates a new `UserDefaultStorage`.
    pub fn new(_: PathBuf) -> Self {
        UserDefaultStorage {}
    }
}
