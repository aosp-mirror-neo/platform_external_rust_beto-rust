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

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use core::fmt::Debug;
use tinyvec::ArrayVec;

/// Error indicating that the buffer capacity has been exceeded.
#[derive(thiserror::Error, Debug)]
#[error("Buffer capacity exceeded")]
pub struct CapacityExceededError;

/// A trait for buffers used in advertisement serialization and deserialization.
#[allow(clippy::len_without_is_empty)]
pub trait AdvBuffer: AsRef<[u8]> + AsMut<[u8]> + Debug + Sized {
    /// Returns the current capacity of the buffer.
    fn capacity(&self) -> usize;

    /// Ensures that the buffer has at least the given capacity.
    /// Returns true if the buffer has enough capacity, false otherwise.
    fn ensure_capacity(&mut self, required_capacity: usize) -> Result<(), CapacityExceededError>;

    /// Returns the currently used length of the buffer.
    /// Implementations should track len separately to ensure
    /// constant time lookup and avoid the need for an `is_empty` method.
    fn len(&self) -> usize;

    /// Sets the currently used length of the buffer.
    /// Panics if new_len > capacity.
    fn set_len(&mut self, new_len: usize);
}

/// A buffer allocated on the stack.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StackBuffer<const N: usize> {
    buffer: ArrayVec<[u8; N]>,
    len: usize,
}

impl<const N: usize> StackBuffer<N> {
    /// Creates a new buffer with the given capacity.
    pub(crate) fn new() -> Self {
        Self { buffer: ArrayVec::from([0u8; N]), len: 0 }
    }
}

impl<const N: usize> AsRef<[u8]> for StackBuffer<N> {
    fn as_ref(&self) -> &[u8] {
        &self.buffer
    }
}

impl<const N: usize> AsMut<[u8]> for StackBuffer<N> {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.buffer
    }
}

impl<const N: usize> AdvBuffer for StackBuffer<N> {
    fn capacity(&self) -> usize {
        N
    }

    fn ensure_capacity(&mut self, required_capacity: usize) -> Result<(), CapacityExceededError> {
        if required_capacity <= N {
            Ok(())
        } else {
            Err(CapacityExceededError)
        }
    }

    fn len(&self) -> usize {
        self.len
    }

    fn set_len(&mut self, new_len: usize) {
        assert!(new_len <= self.capacity(), "new_len exceeds capacity");
        self.len = new_len;
    }
}

/// A buffer allocated on the heap.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct VecBuffer {
    buffer: Vec<u8>,
    len: usize,
}

impl VecBuffer {
    /// Creates a new VecBuffer with the given length.
    pub(crate) fn new(len: usize) -> Self {
        Self { buffer: vec![0; len], len: 0 }
    }
}

impl AsRef<[u8]> for VecBuffer {
    fn as_ref(&self) -> &[u8] {
        &self.buffer
    }
}

impl AsMut<[u8]> for VecBuffer {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.buffer
    }
}

impl AdvBuffer for VecBuffer {
    fn capacity(&self) -> usize {
        self.buffer.capacity()
    }

    fn ensure_capacity(&mut self, required_capacity: usize) -> Result<(), CapacityExceededError> {
        if required_capacity > self.buffer.capacity() {
            self.buffer.reserve(required_capacity - self.buffer.len());
        }

        // ensure the vec is at least this long for AsMut
        if required_capacity > self.buffer.len() {
            self.buffer.resize(required_capacity, 0);
        }
        Ok(())
    }

    fn len(&self) -> usize {
        self.len
    }

    fn set_len(&mut self, new_len: usize) {
        assert!(new_len <= self.capacity(), "new_len exceeds capacity");
        self.len = new_len;
    }
}
