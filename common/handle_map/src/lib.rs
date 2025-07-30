// Copyright 2023 Google LLC
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

//! A thread-safe implementation of a map for managing object handles,
//! a safer alternative to raw pointers for FFI interop.

use core::fmt::Debug;
use core::ops::{Deref, DerefMut};

mod guard;
mod handle;
mod map;
mod owned_handle;

pub(crate) mod shard;

pub mod declare_handle_map;

#[doc(hidden)]
pub mod reexport {
    pub use lazy_static;
}

pub use handle::Handle;
pub use map::{
    HandleMap, HandleMapDimensions, HandleMapFullError, HandleMapTryAllocateError,
    HandleNotPresentError,
};
pub use owned_handle::OwnedHandle;

/// Externally-facing trait for things which behave like handle-map handles
/// with a globally-defined handle-map for the type.
pub trait HandleLike: Sized {
    /// The underlying object type pointed-to by this handle
    type Object: Send + Sync;

    /// Tries to allocate a new handle using the given (fallible)
    /// provider to construct the underlying stored object as
    /// a new entry into the global handle table for this type.
    fn try_allocate<E: Debug>(
        initial_value_provider: impl FnOnce() -> Result<Self::Object, E>,
    ) -> Result<Self, HandleMapTryAllocateError<E>>;

    /// Tries to allocate a new handle using the given (infallible)
    /// provider to construct the underlying stored object as
    /// a new entry into the global handle table for this type.
    fn allocate(
        initial_value_provider: impl FnOnce() -> Self::Object,
    ) -> Result<Self, HandleMapFullError>;

    /// Gets a RAII read-guard on the contents behind this handle.
    fn get(&self) -> Result<impl Deref<Target = Self::Object> + '_, HandleNotPresentError>;

    /// Gets a RAII read-write guard on the contents behind this handle.
    fn get_mut(&self) -> Result<impl DerefMut<Target = Self::Object> + '_, HandleNotPresentError>;

    /// Deallocates the contents behind this handle.
    fn deallocate(self) -> Result<Self::Object, HandleNotPresentError>;
}

#[cfg(test)]
mod tests;
