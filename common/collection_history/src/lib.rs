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

//! A collection that tracks all previous entries that have been inserted and removed and provides a nice-looking output when displayed.

use chrono::{DateTime, Utc};
use prettytable::{Cell, Row, Table};
use std::{collections::HashMap, fmt::Debug, iter::Map};
use thiserror::Error;

/// The insertion for this key-value pair failed.
#[derive(Error, Debug)]
#[error("Key {:?} already exists. Cannot insert value {:?}. Key was originally inserted at {}", key, value, insertion_time.to_rfc3339())]
pub struct InsertError<K: Debug, V: Debug> {
    pub key: K,
    pub value: V,
    insertion_time: DateTime<Utc>,
}

/// The update operation for this key-value pair failed.
#[derive(Error, Debug)]
#[error("Key '{:?}' does not exist. Cannot update value {:?}.", key, value)]
pub struct UpdateError<K: Debug, V: Debug> {
    pub key: K,
    pub value: V,
}

/// The removal operation for this key failed.
#[derive(Error, Debug)]
pub enum RemoveError<K: Debug, V: Debug> {
    #[error(
        "Key '{:?}' does not exist. Key was originally removed at time {:?} with value {:?}",
        key,
        old_value,
        removal_time.to_rfc3339()
    )]
    KeyAlreadyRemoved {
        key: K,
        old_value: V,
        removal_time: DateTime<Utc>,
    },
    #[error("Key '{:?}' does not exist", key)]
    KeyDoesNotExist { key: K },
}

/// The get operation for this key failed.
#[derive(Error, Debug)]
pub enum GetError<K: Debug, V: Debug> {
    #[error(
        "Key '{:?}' does not exist. Key was originally removed at time {:?} with value {:?}",
        key,
        old_value,
        removal_time.to_rfc3339()
    )]
    KeyAlreadyRemoved {
        key: K,
        old_value: V,
        removal_time: DateTime<Utc>,
    },
    #[error("Key '{:?}' does not exist", key)]
    KeyDoesNotExist { key: K },
}

#[doc(hidden)]
#[derive(Clone, Debug)]
pub struct Session<K, V> {
    key: K,
    value: V,
    insertion_time: DateTime<Utc>,
    last_update_time: DateTime<Utc>,
    removal_time: Option<DateTime<Utc>>,
}

impl<K, V> Session<K, V> {
    fn new(key: K, value: V) -> Self {
        let now = Utc::now();
        Self {
            key,
            value,
            insertion_time: now,
            last_update_time: now,
            removal_time: None,
        }
    }

    fn set_value(&mut self, value: V) -> V {
        let old_value = std::mem::replace(&mut self.value, value);
        self.update_last_updated_time();
        old_value
    }

    fn update_last_updated_time(&mut self) {
        self.last_update_time = Utc::now();
    }

    fn update_removal_time(&mut self) {
        self.update_last_updated_time();
        self.removal_time = Some(Utc::now());
    }

    fn get_duration(&self) -> i64 {
        let end_time = self.removal_time.unwrap_or_else(Utc::now);
        end_time.timestamp_millis() - self.insertion_time.timestamp_millis()
    }
}

enum EventStatus {
    Active,
    Complete,
}

impl std::fmt::Display for EventStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventStatus::Active => write!(f, "ACTIVE"),
            EventStatus::Complete => write!(f, "COMPLETE"),
        }
    }
}

/// A map that tracks when keys were added and removed.
#[derive(Debug, Clone)]
pub struct CollectionHistory<K, V> {
    max_history: usize,
    past_sessions: Vec<Session<K, V>>,
    active_sessions: HashMap<K, Session<K, V>>,
}

impl<K, V> CollectionHistory<K, V>
where
    K: Eq + std::hash::Hash + Clone + std::fmt::Debug,
    V: std::fmt::Debug + Clone,
{
    /// Creates a new [CollectionHistory] with a maximum number of `max_history` historical entries tracked.
    pub fn new(max_history: usize) -> Self {
        Self {
            max_history,
            past_sessions: Vec::new(),
            active_sessions: HashMap::new(),
        }
    }

    /// Inserts a key-value pair. If the key already exists, an InsertError with the key and value is returned.
    pub fn insert(&mut self, key: K, value: V) -> Result<(), InsertError<K, V>> {
        if let Some(session) = self.active_sessions.get(&key) {
            return Err(InsertError {
                key,
                value,
                insertion_time: session.insertion_time,
            });
        }

        self.active_sessions
            .insert(key.clone(), Session::new(key, value));
        self.prune_old_sessions();
        Ok(())
    }

    /// Updates the key to the new value, returning the old value if successful.
    pub fn update(&mut self, key: K, value: V) -> Result<V, UpdateError<K, V>> {
        if let Some(session) = self.active_sessions.get_mut(&key) {
            return Ok(session.set_value(value));
        }
        Err(UpdateError { key, value })
    }

    /// Removes the value corresponding to key, if it exists.
    pub fn remove(&mut self, key: K) -> Result<V, RemoveError<K, V>> {
        if let Some(mut session) = self.active_sessions.remove(&key) {
            session.update_removal_time();
            let value = session.value.clone();
            self.past_sessions.insert(0, session);
            self.prune_old_sessions();
            Ok(value)
        } else if let Some(session) = self.find_old_session(&key) {
            return Err(RemoveError::KeyAlreadyRemoved {
                key,
                old_value: session.value.clone(),
                removal_time: session.last_update_time,
            });
        } else {
            return Err(RemoveError::KeyDoesNotExist { key });
        }
    }

    /// Returns true if key is currently active.
    pub fn contains_key(&self, key: &K) -> bool {
        self.active_sessions.contains_key(key)
    }

    /// Returns the value corresponding to `key` if it exists.
    pub fn get(&self, key: &K) -> Result<&V, GetError<K, &V>> {
        if let Some(session) = self.active_sessions.get(key) {
            Ok(&session.value)
        } else if let Some(session) = self.find_old_session(key) {
            Err(GetError::KeyAlreadyRemoved {
                key: key.clone(),
                old_value: &session.value,
                removal_time: session.last_update_time,
            })
        } else {
            Err(GetError::KeyDoesNotExist { key: key.clone() })
        }
    }

    /// Clears this [CollectionHistory].
    pub fn clear(&mut self) {
        let keys: Vec<K> = self.active_sessions.keys().cloned().collect();
        for key in keys {
            let _ = self.remove(key);
        }
    }

    /// Returns an iterator over the active keys in this [CollectionHistory].
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.active_sessions.keys()
    }

    /// Returns an iterator over the active values in this [CollectionHistory].
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.active_sessions.values().map(|s| &s.value)
    }

    /// Returns an iterator over the active entries in this [CollectionHistory].
    pub fn entry_set(&self) -> impl Iterator<Item = (&K, &V)> {
        self.active_sessions.iter().map(|(k, s)| (k, &s.value))
    }

    /// Returns the number of entries in this [CollectionHistory].
    pub fn len(&self) -> usize {
        self.active_sessions.len()
    }

    /// Returns true if there are no active sessions in this [CollectionHistory].
    pub fn is_empty(&self) -> bool {
        self.active_sessions.is_empty()
    }

    fn history_size(&self) -> usize {
        self.past_sessions.len() + self.len()
    }

    fn prune_old_sessions(&mut self) {
        if self.active_sessions.len() >= self.max_history {
            self.past_sessions.clear();
            return;
        }

        while self.history_size() > self.max_history {
            self.past_sessions.remove(0);
        }
    }

    fn find_old_session(&self, key: &K) -> Option<&Session<K, V>> {
        self.past_sessions.iter().find(|s| &s.key == key)
    }
}

impl<K, V> IntoIterator for CollectionHistory<K, V> {
    type Item = (K, V);
    type IntoIter = Map<
        std::collections::hash_map::IntoIter<K, Session<K, V>>,
        fn((K, Session<K, V>)) -> Self::Item,
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.active_sessions.into_iter().map(|(k, s)| (k, s.value))
    }
}

impl<K: Debug, V: Debug> std::fmt::Display for CollectionHistory<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut sorted_sessions: Vec<&Session<K, V>> = self.active_sessions.values().collect();
        sorted_sessions.sort_by(|a, b| b.last_update_time.cmp(&a.last_update_time));

        let mut table = Table::new();
        table.add_row(Row::new(vec![
            Cell::new("Key"),
            Cell::new("Value"),
            Cell::new("Status"),
            Cell::new("Start Time"),
            Cell::new("End Time"),
            Cell::new("Duration"),
        ]));

        for session in sorted_sessions {
            table.add_row(Row::new(vec![
                Cell::new(&format!("{:?}", &session.key)),
                Cell::new(&format!("{:?}", &session.value)),
                Cell::new(&EventStatus::Active.to_string()),
                Cell::new(&session.insertion_time.to_rfc3339()),
                Cell::new(&session.last_update_time.to_rfc3339()),
                Cell::new(&format!("{}ms", session.get_duration())),
            ]));
        }

        for session in &self.past_sessions {
            table.add_row(Row::new(vec![
                Cell::new(&format!("{:?}", &session.key)),
                Cell::new(&format!("{:?}", &session.value)),
                Cell::new(&EventStatus::Complete.to_string()),
                Cell::new(&session.insertion_time.to_rfc3339()),
                Cell::new(&session.last_update_time.to_rfc3339()),
                Cell::new(&format!("{}ms", session.get_duration())),
            ]));
        }

        write!(f, "{table}")
    }
}
