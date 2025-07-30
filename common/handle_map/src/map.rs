// Copyright 2024 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::guard::{Downcast, LookupByHandle, ReadGuard, ReadWriteGuard};
use crate::shard::{
    BoxedAnyProvider, HandleMapShard, ShardAllocationError, ShardValue, ValueProvider,
};
use crate::Handle;

use core::any::Any;
use core::convert::Infallible;
use core::fmt::Debug;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Error raised when attempting to allocate into a full handle-map.
#[derive(Debug)]
pub struct HandleMapFullError;

/// Error raised when the entry for a given [`Handle`] doesn't exist.
#[derive(Debug)]
pub struct HandleNotPresentError;

/// Errors which may be raised while attempting to allocate
/// a handle from contents given by a (fallible) value-provider.
#[derive(Debug)]
pub enum HandleMapTryAllocateError<E: Debug> {
    /// The call to the value-provider for the allocation failed.
    ValueProviderFailed(E),
    /// We couldn't reserve a spot for the allocation, because
    /// the handle-map was full.
    HandleMapFull,
}

/// FFI-transmissible structure expressing the dimensions
/// (max # of allocatable slots, number of shards) of a handle-map
/// to be used upon initialization.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct HandleMapDimensions {
    /// The number of shards which are employed
    /// by the associated handle-map.
    pub num_shards: u8,
    /// The maximum number of active handles which may be
    /// stored within the associated handle-map.
    pub max_active_handles: u32,
}

/// A thread-safe mapping from "handle"s [like pointers, but safer]
/// to underlying structures, supporting allocations, reads, writes,
/// and deallocations of objects behind handles.
pub struct HandleMap<T> {
    /// Internal type-erased implementation. This allows most of the code to be shared for
    /// code-size reasons.
    erased: ErasedHandleMap,
    /// Keep `T` around so that we can downcast values from the erased implementation.
    _t: PhantomData<T>,
}

impl<T: Any + Send + Sync + 'static> HandleMap<T> {
    /// Creates a new handle-map with the given `HandleMapDimensions`.
    pub fn with_dimensions(dimensions: HandleMapDimensions) -> Self {
        Self {
            erased: ErasedHandleMap::with_dimensions(dimensions),
            _t: PhantomData,
        }
    }

    /// Allocates a new object within the given handle-map, returning
    /// a handle to the location it was stored at. This operation
    /// may fail if attempting to allocate over the `dimensions.max_active_handles`
    /// limit imposed on the handle-map, in which case this method
    /// will return a `HandleMapFullError`.
    ///
    /// If you want the passed closure to be able to possibly fail, see
    /// [`Self::try_allocate`] instead.
    pub fn allocate(
        &self,
        initial_value_provider: impl FnOnce() -> T,
    ) -> Result<Handle, HandleMapFullError> {
        let value_provider =
            BoxedAnyProvider(move || Ok::<_, Infallible>(initial_value_provider()));
        self.erased
            .try_allocate_impl(value_provider)
            .map_err(|e| match e {
                HandleMapTryAllocateError::ValueProviderFailed(never) => match never {},
                HandleMapTryAllocateError::HandleMapFull => HandleMapFullError,
            })
    }

    /// Attempts to allocate a new object within the given handle-map, returning
    /// a handle to the location it was stored at.
    ///
    /// This operation may fail if attempting to allocate over the `dimensions.max_active_handles`
    /// limit imposed on the handle-map, in which case this method will return a
    /// `HandleMapTryAllocateError::HandleMapFull` and the given `initial_value_provider` will not
    /// be run.
    ///
    /// The passed initial-value provider may also fail, in which case this will return the error
    /// wrapped in `HandleMapTryAllocateError::ValueProviderFailed`.
    ///
    /// If your initial-value provider is infallible, see [`Self::allocate`] instead.
    pub fn try_allocate<E: Debug>(
        &self,
        initial_value_provider: impl FnOnce() -> Result<T, E>,
    ) -> Result<Handle, HandleMapTryAllocateError<E>> {
        let value_provider = BoxedAnyProvider(initial_value_provider);
        self.erased.try_allocate_impl(value_provider)
    }

    /// Gets a read-only reference to an object within the given handle-map,
    /// if the given handle is present. Otherwise, returns [`HandleNotPresentError`].
    pub fn get(
        &self,
        handle: Handle,
    ) -> Result<impl Deref<Target = T> + '_, HandleNotPresentError> {
        self.erased
            .get(handle)
            .map(|read_guard| read_guard.map(Downcast::<T>::new()))
    }

    /// Gets a read+write reference to an object within the given handle-map,
    /// if the given handle is present. Otherwise, returns [`HandleNotPresentError`].
    pub fn get_mut(
        &self,
        handle: Handle,
    ) -> Result<impl DerefMut<Target = T> + '_, HandleNotPresentError> {
        self.erased
            .get_mut(handle)
            .map(|read_write_guard| read_write_guard.map(Downcast::<T>::new()))
    }

    /// Removes the object pointed to by the given handle in
    /// the handle-map, returning the removed object if it
    /// exists. Otherwise, returns [`HandleNotPresentError`].
    pub fn deallocate(&self, handle: Handle) -> Result<T, HandleNotPresentError> {
        // Values should always match T
        #[allow(clippy::expect_used)]
        let downcast = |boxed: ShardValue| {
            *boxed
                .downcast::<T>()
                .expect("Handle map value has invalid type")
        };
        self.erased.deallocate(handle).map(downcast)
    }

    /// Gets the actual number of elements stored in the entire map.
    pub fn get_current_allocation_count(&self) -> u32 {
        self.erased.get_current_allocation_count()
    }

    /// Sets the new-handle-id counter to the given value.
    /// Only suitable for tests.
    #[cfg(test)]
    pub(crate) fn set_new_handle_id_counter(&mut self, value: u64) {
        self.erased.set_new_handle_id_counter(value)
    }
}

/// Internal type-erased handle map implementation.
struct ErasedHandleMap {
    /// The dimensions of this handle-map
    dimensions: HandleMapDimensions,

    /// The individually-lockable "shards" of the handle-map,
    /// among which the keys will be roughly uniformly-distributed.
    handle_map_shards: Box<[HandleMapShard]>,

    /// An atomically-incrementing counter which tracks the
    /// next handle ID which allocations will attempt to use.
    new_handle_id_counter: AtomicU64,

    /// An atomic integer roughly tracking the number of
    /// currently-outstanding allocated entries in this
    /// handle-map among all [`HandleMapShard`]s.
    outstanding_allocations_counter: AtomicU32,
}

impl ErasedHandleMap {
    /// Creates a new handle-map with the given `HandleMapDimensions`.
    fn with_dimensions(dimensions: HandleMapDimensions) -> Self {
        let mut handle_map_shards = Vec::with_capacity(dimensions.num_shards as usize);
        for _ in 0..dimensions.num_shards {
            handle_map_shards.push(HandleMapShard::default());
        }
        let handle_map_shards = handle_map_shards.into_boxed_slice();
        Self {
            dimensions,
            handle_map_shards,
            new_handle_id_counter: AtomicU64::new(0),
            outstanding_allocations_counter: AtomicU32::new(0),
        }
    }

    /// Try allocate using a type erased ValueProvider.
    fn try_allocate_impl<E: Debug>(
        &self,
        mut value_provider: impl ValueProvider<E>,
    ) -> Result<Handle, HandleMapTryAllocateError<E>> {
        loop {
            // Increment the new-handle-ID counter using relaxed memory ordering,
            // since the only invariant that we want to enforce is that concurrently-running
            // threads always get distinct new handle-ids.
            let new_handle_id = self.new_handle_id_counter.fetch_add(1, Ordering::Relaxed);
            let new_handle = Handle::from_id(new_handle_id);

            let shard_allocate_result = self.get_shard_from_handle(new_handle).try_allocate(
                new_handle,
                value_provider,
                &self.outstanding_allocations_counter,
                self.dimensions.max_active_handles,
            );
            match shard_allocate_result {
                Ok(_) => {
                    return Ok(new_handle);
                }
                Err(ShardAllocationError::ValueProviderFailed(e)) => {
                    return Err(HandleMapTryAllocateError::ValueProviderFailed(e))
                }
                Err(ShardAllocationError::ExceedsAllocationLimit) => {
                    return Err(HandleMapTryAllocateError::HandleMapFull);
                }
                Err(ShardAllocationError::EntryOccupied(thrown_back_provider)) => {
                    // We need to do the whole thing again with a new ID
                    value_provider = thrown_back_provider;
                }
            }
        }
    }

    fn get(&self, handle: Handle) -> Result<ReadGuard<LookupByHandle>, HandleNotPresentError> {
        self.get_shard_from_handle(handle).get(handle)
    }

    fn get_mut(
        &self,
        handle: Handle,
    ) -> Result<ReadWriteGuard<LookupByHandle>, HandleNotPresentError> {
        self.get_shard_from_handle(handle).get_mut(handle)
    }

    /// Removes the object pointed to by the given handle in
    /// the handle-map, returning the removed object if it
    /// exists. Otherwise, returns [`HandleNotPresentError`].
    fn deallocate(&self, handle: Handle) -> Result<ShardValue, HandleNotPresentError> {
        self.get_shard_from_handle(handle)
            .deallocate(handle, &self.outstanding_allocations_counter)
    }

    /// Lookup the shard containing the given handle.
    fn get_shard_from_handle(&self, handle: Handle) -> &HandleMapShard {
        let shard_index = handle.get_shard_index(self.dimensions.num_shards);
        #[allow(clippy::expect_used)]
        self.handle_map_shards
            .get(shard_index)
            // This should never occur due to `get_shard_index` respecting the given `num_shards`.
            .expect("shard_index is always in range")
    }

    /// Gets the actual number of elements stored in the entire map.
    pub fn get_current_allocation_count(&self) -> u32 {
        self.outstanding_allocations_counter.load(Ordering::Relaxed)
    }

    /// Sets the new-handle-id counter to the given value.
    /// Only suitable for tests.
    #[cfg(test)]
    pub(crate) fn set_new_handle_id_counter(&mut self, value: u64) {
        self.new_handle_id_counter = AtomicU64::new(value);
    }
}
