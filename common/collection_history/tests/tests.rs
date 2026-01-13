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

use collection_history::CollectionHistory;
use googletest::prelude::*;

#[gtest]
fn insert_remove_update_fails() {
    let mut map = CollectionHistory::<u64, String>::new(10);

    assert!(map.insert(1, "Hello".to_string()).is_ok());
    assert!(map.remove(1).is_ok());
    assert!(map.update(1, "Bye".to_string()).is_err());
    assert!(map.is_empty())
}

#[gtest]
fn insert_remove_insert() {
    let mut map = CollectionHistory::<u64, String>::new(10);

    assert!(map.insert(1, "Hello".to_string()).is_ok());
    assert!(map.remove(1).is_ok());
    assert!(map.insert(1, "Bye".to_string()).is_ok());
    assert_that!(
        map.entry_set()
            .map(|(k, v)| (*k, v.clone()))
            .collect::<Vec<(u64, String)>>(),
        contains(eq(&(1, "Bye".to_string())))
    )
}

#[gtest]
fn remove_remove_fails() {
    let mut map = CollectionHistory::<u64, String>::new(10);

    assert!(map.insert(1, "Hello".to_string()).is_ok());
    assert!(map.remove(1).is_ok());
    assert!(map.remove(1).is_err());
    assert!(map.is_empty())
}

#[gtest]
fn clear() {
    let mut map = CollectionHistory::<u64, String>::new(10);

    assert!(map.insert(1, "Hello".to_string()).is_ok());
    map.clear();
    assert_that!(map.len(), eq(0));
}

#[gtest]
fn history_limit_does_not_apply_to_active() {
    let mut map = CollectionHistory::<u64, String>::new(1);
    assert!(map.insert(1, "Hello".to_string()).is_ok());
    assert!(map.insert(2, "Bye".to_string()).is_ok());

    assert_that!(map.len(), eq(2));
}

#[gtest]
fn update_returns_old_value() {
    let mut map = CollectionHistory::<u64, String>::new(1);
    assert!(map.insert(1, "Hello".to_string()).is_ok());
    assert_that!(
        map.update(1, "Bye".to_string()),
        matches_pattern!(Ok("Hello"))
    );
}

/*
#[gtest]
fn print_format() -> Result<()> {
    let mut map = CollectionHistory::<u64, String>::new(1);
    assert!(map.insert(1, "Hello".to_string()).is_ok());
    assert!(map.insert(2, "Bye".to_string()).is_ok());

    eprintln!("{}", map);
    fail!()
}
*/
