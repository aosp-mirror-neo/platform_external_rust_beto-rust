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

use crate::{HandleLike, HandleNotPresentError};
use core::{
    marker::Copy,
    ops::{Deref, DerefMut},
};

/// An owned handle that should be deallocated when dropped.
pub struct OwnedHandle<H: HandleLike + Copy>(H);

impl<H: HandleLike + Copy> Deref for OwnedHandle<H> {
    type Target = H;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<H: HandleLike + Copy> DerefMut for OwnedHandle<H> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<H: HandleLike + Copy> OwnedHandle<H> {
    /// Create an owned handle from an existing handle
    pub fn new(handle: H) -> Self {
        Self(handle)
    }

    /// Convert from an owned handle back to shared handle.
    pub fn into_shared(self) -> H {
        let skip_drop = core::mem::ManuallyDrop::new(self);
        skip_drop.0
    }

    /// Deallocate the inner value
    pub fn deallocate(self) -> Result<<H as HandleLike>::Object, HandleNotPresentError> {
        self.into_shared().deallocate()
    }
}

impl<H: HandleLike + Copy> Drop for OwnedHandle<H> {
    fn drop(&mut self) {
        let _ = self.0.deallocate();
    }
}

#[cfg(test)]
#[allow(dead_code)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{declare_handle_map, HandleLike, HandleMapDimensions, HandleNotPresentError};

    #[repr(C)]
    #[derive(Clone, Copy, PartialEq, Eq)]
    struct MyHandle {
        handle_id: u64,
    }

    struct MyValue {
        name: String,
    }

    static DIMENSIONS: HandleMapDimensions = HandleMapDimensions {
        num_shards: 2,
        max_active_handles: u32::MAX - 1,
    };

    declare_handle_map!(DIMENSIONS, MyHandle, MyValue);

    #[test]
    fn owned_handle_will_free() {
        let h = MyHandle::allocate(|| MyValue {
            name: "Hello".to_owned(),
        })
        .unwrap();

        {
            let _owned = OwnedHandle::new(h);
        }

        assert!(matches!(h.get(), Err(HandleNotPresentError)));
    }

    #[test]
    fn owned_handle_deallocate_will_free() {
        let h = MyHandle::allocate(|| MyValue {
            name: "Hello".to_owned(),
        })
        .unwrap();

        let owned = OwnedHandle::new(h);
        let _ = owned.deallocate();

        assert!(matches!(h.get(), Err(HandleNotPresentError)));
    }

    #[test]
    fn owned_handle_into_shared_will_not_free() {
        let h = MyHandle::allocate(|| MyValue {
            name: "Hello".to_owned(),
        })
        .unwrap();

        let h2 = {
            let owned = OwnedHandle::new(h);
            owned.into_shared()
        };

        assert!(!matches!(h.get(), Err(HandleNotPresentError)));
        assert!(!matches!(h2.get(), Err(HandleNotPresentError)));
    }
}
