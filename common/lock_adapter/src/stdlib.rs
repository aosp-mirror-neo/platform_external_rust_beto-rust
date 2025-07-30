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

use crate::mapped_guard::{MappedGuard, RwMapping};
use crate::NoPoisonMutex;
use core::ops::{Deref, DerefMut};

/// A mutual exclusion primitive useful for protecting shared data
pub struct Mutex<T>(std::sync::Mutex<T>);

impl<T> NoPoisonMutex<T> for Mutex<T> {
    type MutexGuard<'a>
        = std::sync::MutexGuard<'a, T>
    where
        T: 'a;

    fn lock(&self) -> Self::MutexGuard<'_> {
        self.0.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    fn try_lock(&self) -> Option<Self::MutexGuard<'_>> {
        match self.0.try_lock() {
            Ok(guard) => Some(guard),
            Err(std::sync::TryLockError::Poisoned(guard)) => Some(guard.into_inner()),
            Err(std::sync::TryLockError::WouldBlock) => None,
        }
    }

    fn new(value: T) -> Self {
        Self(std::sync::Mutex::new(value))
    }
}

/// A reader-writer lock
/// This type of lock allows a number of readers or at most one writer at any point in time.
/// The write portion of this lock typically allows modification of the underlying data (exclusive access)
/// and the read portion of this lock typically allows for read-only access (shared access).
pub struct RwLock<T>(std::sync::RwLock<T>);

impl<T> RwLock<T> {
    /// Creates a new instance of an `RwLock<T>` which is unlocked.
    pub const fn new(value: T) -> Self {
        Self(std::sync::RwLock::new(value))
    }
}

impl<T> crate::RwLock<T> for RwLock<T> {
    type RwLockReadGuard<'a>
        = RwLockReadGuard<'a, T>
    where
        T: 'a;

    type RwLockWriteGuard<'a>
        = RwLockWriteGuard<'a, T>
    where
        T: 'a;

    fn read(&self) -> Self::RwLockReadGuard<'_> {
        RwLockReadGuard(self.0.read().unwrap_or_else(|e| e.into_inner()))
    }

    fn write(&self) -> Self::RwLockWriteGuard<'_> {
        RwLockWriteGuard(self.0.write().unwrap_or_else(|e| e.into_inner()))
    }
}

/// RAII structure used to release the shared read access of a lock when dropped.
pub struct RwLockReadGuard<'a, T>(std::sync::RwLockReadGuard<'a, T>);

impl<'a, T> RwLockReadGuard<'a, T> {
    /// Make a new MappedRwLockReadGuard for a component of the locked data.
    pub fn map<M>(s: Self, mapping: M) -> MappedRwLockReadGuard<'a, M>
    where
        M: RwMapping<Arg = T>,
    {
        MappedRwLockReadGuard::new(s, mapping)
    }
}

impl<T> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

/// RAII structure used to release the exclusive write access of a lock when dropped.
pub struct RwLockWriteGuard<'a, T>(std::sync::RwLockWriteGuard<'a, T>);

impl<'a, T> RwLockWriteGuard<'a, T> {
    /// Make a new MappedRwLockWriteGuard for a component of the locked data.
    pub fn map<M>(s: Self, mapping: M) -> MappedRwLockWriteGuard<'a, M>
    where
        M: RwMapping<Arg = T>,
    {
        MappedRwLockWriteGuard::new(s, mapping)
    }
}

impl<T> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl<T> DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.deref_mut()
    }
}

/// An RAII read lock guard returned by RwLockReadGuard::map, which can point to a subfield of the protected data.
pub type MappedRwLockReadGuard<'a, M> = MappedGuard<RwLockReadGuard<'a, <M as RwMapping>::Arg>, M>;

/// An RAII read lock guard returned by RwLockWriteGuard::map, which can point to a subfield of the protected data.
pub type MappedRwLockWriteGuard<'a, M> =
    MappedGuard<RwLockWriteGuard<'a, <M as RwMapping>::Arg>, M>;
