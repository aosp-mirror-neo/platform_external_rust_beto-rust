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

use core::any::Any;
use core::ops::{Deref, DerefMut};
use lock_adapter::RwLock as _;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::guard::{
    LookupByHandle, RawReadGuard, RawReadWriteGuard, ReadGuard, ReadWriteGuard, RwLock,
};
use crate::{Handle, HandleNotPresentError};

// Bunch o' type aliases to make talking about them much easier in the shard code.
pub type ShardValue = Box<dyn Any + Send + Sync + 'static>;
pub type ShardMap = HashMap<Handle, ShardValue>;

type ShardReadWriteLock = RwLock<ShardMap>;
type ShardReadGuard<'a> = RawReadGuard<'a, ShardMap>;
type ShardReadWriteGuard<'a> = RawReadWriteGuard<'a, ShardMap>;

pub(crate) trait ValueProvider<E> {
    fn provide_value(self) -> Result<ShardValue, E>;
}

impl<E, F: FnOnce() -> Result<ShardValue, E>> ValueProvider<E> for F {
    fn provide_value(self) -> Result<ShardValue, E> {
        self()
    }
}

pub(crate) struct BoxedAnyProvider<F>(pub F);

impl<T: Any + Send + Sync + 'static, E, F: FnOnce() -> Result<T, E>> ValueProvider<E>
    for BoxedAnyProvider<F>
{
    fn provide_value(self) -> Result<ShardValue, E> {
        match self.0() {
            Ok(value) => Ok(Box::new(value)),
            Err(e) => Err(e),
        }
    }
}

/// Internal error enum for failed allocations into a given shard.
pub(crate) enum ShardAllocationError<E, P: ValueProvider<E>> {
    /// Error for when the entry for the handle is occupied,
    /// in which case we spit out the object-provider to try again
    /// with a new handle-id.
    EntryOccupied(P),
    /// Error for when we would exceed the maximum number of allocations.
    ExceedsAllocationLimit,
    /// Error for when the initial value-provider call failed.
    ValueProviderFailed(E),
}

/// An individual handle-map shard, which is ultimately
/// just a hash-map behind a lock.
pub(crate) struct HandleMapShard {
    data: ShardReadWriteLock,
}

impl Default for HandleMapShard {
    fn default() -> Self {
        Self {
            data: ShardReadWriteLock::new(HashMap::new()),
        }
    }
}

impl HandleMapShard {
    pub fn get(&self, handle: Handle) -> Result<ReadGuard<LookupByHandle>, HandleNotPresentError> {
        let map_read_guard = ShardReadWriteLock::read(&self.data);
        let read_only_map_ref = map_read_guard.deref();
        if read_only_map_ref.contains_key(&handle) {
            let object_read_guard =
                ShardReadGuard::map(map_read_guard, LookupByHandle::new(handle));
            Ok(object_read_guard)
        } else {
            // Auto-drop the read guard, and return an error
            Err(HandleNotPresentError)
        }
    }
    /// Gets a read-write guard on the entire shard map if an entry for the given
    /// handle exists, but if not, yield [`HandleNotPresentError`].
    fn get_read_write_guard_if_entry_exists(
        &self,
        handle: Handle,
    ) -> Result<ShardReadWriteGuard, HandleNotPresentError> {
        let contains_key = {
            let map_ref = self.data.read();
            map_ref.contains_key(&handle)
        };
        if contains_key {
            // If we know that the entry exists, and we're currently
            // holding a read-lock, we know that we're safe to request
            // an upgrade to a write lock, since only one write or
            // upgradable read lock can be outstanding at any one time.
            let write_guard = self.data.write();
            Ok(write_guard)
        } else {
            // Auto-drop the read guard, we don't need to allow a write.
            Err(HandleNotPresentError)
        }
    }

    pub fn get_mut(
        &self,
        handle: Handle,
    ) -> Result<ReadWriteGuard<LookupByHandle>, HandleNotPresentError> {
        let map_read_write_guard = self.get_read_write_guard_if_entry_exists(handle)?;
        // Expose only the pointed-to object with a mapped read-write guard
        let object_read_write_guard =
            ShardReadWriteGuard::map(map_read_write_guard, LookupByHandle::new(handle));
        Ok(object_read_write_guard)
    }

    pub fn deallocate(
        &self,
        handle: Handle,
        outstanding_allocations_counter: &AtomicU32,
    ) -> Result<ShardValue, HandleNotPresentError> {
        let mut map_read_write_guard = self.get_read_write_guard_if_entry_exists(handle)?;
        // We don't need to worry about double-decrements, since the above call
        // got us an upgradable read guard for our read, which means it's the only
        // outstanding upgradeable guard on the shard. See `spin` documentation.
        // Remove the pointed-to object from the map, and return it,
        // releasing the lock when the guard goes out of scope.
        #[allow(clippy::expect_used)]
        let removed_object = map_read_write_guard
            .deref_mut()
            .remove(&handle)
            .expect("existence of handle is checked above");
        // Decrement the allocations counter. Release ordering because we want
        // to ensure that clearing the map entry never gets re-ordered to after when
        // this counter gets decremented.
        let _ = outstanding_allocations_counter.fetch_sub(1, Ordering::Release);
        Ok(removed_object)
    }

    pub fn try_allocate<E, P: ValueProvider<E>>(
        &self,
        handle: Handle,
        object_provider: P,
        outstanding_allocations_counter: &AtomicU32,
        max_active_handles: u32,
    ) -> Result<(), ShardAllocationError<E, P>> {
        let mut read_write_guard = self.data.write();
        match read_write_guard.entry(handle) {
            Occupied(_) => {
                // We've already allocated for that handle-id, so yield
                // the object provider back to the caller.
                Err(ShardAllocationError::EntryOccupied(object_provider))
            }
            Vacant(vacant_entry) => {
                // An entry is open, but we haven't yet checked the allocations count.
                // Try to increment the total allocations count atomically.
                // Use acquire ordering on a successful bump, because we don't want
                // to invoke the allocation closure before we have a guaranteed slot.
                // On the other hand, upon failure, we don't care about ordering
                // of surrounding operations, and so we use a relaxed ordering there.
                let allocation_count_bump_result = outstanding_allocations_counter.fetch_update(
                    Ordering::Acquire,
                    Ordering::Relaxed,
                    |old_total_allocations| {
                        if old_total_allocations >= max_active_handles {
                            None
                        } else {
                            Some(old_total_allocations + 1)
                        }
                    },
                );
                match allocation_count_bump_result {
                    Ok(_) => {
                        // We're good to actually allocate,
                        // so attempt to call the value-provider.
                        match object_provider.provide_value() {
                            Ok(object) => {
                                // Successfully obtained the initial value,
                                // so insert it into the vacant entry.
                                let _ = vacant_entry.insert(object);
                                Ok(())
                            }
                            Err(e) => Err(ShardAllocationError::ValueProviderFailed(e)),
                        }
                    }
                    Err(_) => {
                        // The allocation would cause us to exceed the allowed allocations,
                        // so release all locks and error.
                        Err(ShardAllocationError::ExceedsAllocationLimit)
                    }
                }
            }
        }
    }
}
