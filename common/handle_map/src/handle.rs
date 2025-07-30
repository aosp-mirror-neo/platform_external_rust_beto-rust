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

/// An individual handle to be given out by a [`HandleMap`][crate::HandleMap].
/// This representation is untyped, and just a wrapper
/// around a handle-id, in contrast to implementors of [`HandleLike`][crate::HandleLike].
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Handle {
    handle_id: u64,
}

impl From<&Handle> for Handle {
    fn from(handle: &Handle) -> Self {
        *handle
    }
}

impl Handle {
    /// Constructs a handle wrapping the given ID.
    ///
    /// No validity checks are done on the wrapped ID
    /// to ensure that the given ID is active in
    /// any specific handle-map, and the type
    /// of the handle is not represented.
    ///
    /// As a result, this method is only useful for
    /// allowing access to handles from an FFI layer.
    pub fn from_id(handle_id: u64) -> Self {
        Self { handle_id }
    }

    /// Gets the ID for this handle.
    ///
    /// Since the underlying handle is un-typed,`
    /// this method is only suitable for
    /// transmitting handles across an FFI layer.
    pub fn get_id(&self) -> u64 {
        self.handle_id
    }

    /// Derives the shard index from the handle id
    pub(crate) fn get_shard_index(&self, num_shards: u8) -> usize {
        (self.handle_id % (num_shards as u64)) as usize
    }
}
