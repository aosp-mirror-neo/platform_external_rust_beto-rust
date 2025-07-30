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

use crate::shard::{ShardMap, ShardValue};
use crate::Handle;

use lock_adapter::mapped_guard::RwMapping;
use std::marker::PhantomData;

/// A generic mapped read guard implementation.
pub(crate) use lock_adapter::stdlib::MappedRwLockReadGuard as ReadGuard;

/// A generic mapped read-write guard implementation.
pub(crate) use lock_adapter::stdlib::MappedRwLockWriteGuard as ReadWriteGuard;

/// The RwLock implementation to use
pub(crate) use lock_adapter::stdlib::RwLock;

/// The RwLock read guard implementation to use. Needs to be mapped to a value before being passed
/// to clients.
pub(crate) use lock_adapter::stdlib::RwLockReadGuard as RawReadGuard;

/// The RwLock read-write guard implementation to use. Needs to be mapped to a value before being
/// passed to clients.
pub(crate) use lock_adapter::stdlib::RwLockWriteGuard as RawReadWriteGuard;

/// An [`RwMapping`] implementation that looks up a type-erased [`ShardValue`] from a [`ShardMap`]
/// with a given [`Handle`].
pub struct LookupByHandle {
    handle: Handle,
}

impl LookupByHandle {
    /// Create a mapping from the given [`Handle`]. This handle must exist in the guard's contained
    /// [`ShardMap`]; if not, the mapping functions may panic.
    pub fn new(handle: Handle) -> Self {
        Self { handle }
    }
}

impl RwMapping for LookupByHandle {
    type Arg = ShardMap;
    type Ret = ShardValue;

    fn map<'a>(&self, arg: &'a Self::Arg) -> &'a Self::Ret {
        #[allow(clippy::expect_used)]
        arg.get(&self.handle)
            .expect("Caller must verify that provided handle exists")
    }

    fn map_mut<'a>(&self, arg: &'a mut Self::Arg) -> &'a mut Self::Ret {
        #[allow(clippy::expect_used)]
        arg.get_mut(&self.handle)
            .expect("Caller must verify that provided handle exists")
    }
}

/// An [`RwMapping`] implementation that performs downcast on a type-erased [`ShardValue`]
/// instance. This is used to undo type erasure before giving a guard to a client. Type-erased
/// values are always `'static`.
pub struct Downcast<T> {
    /// Keep `T` around for use in `impl RwMapping`.
    _t: PhantomData<fn(T) -> T>,
}

impl<T: 'static> Downcast<T> {
    pub fn new() -> Self {
        Self { _t: PhantomData }
    }
}

impl<T: 'static> RwMapping for Downcast<T> {
    type Arg = ShardValue;
    type Ret = T;

    fn map<'a>(&self, arg: &'a Self::Arg) -> &'a Self::Ret {
        #[allow(clippy::expect_used)]
        arg.downcast_ref::<T>()
            .expect("Caller must verify the type is correct")
    }

    fn map_mut<'a>(&self, arg: &'a mut Self::Arg) -> &'a mut Self::Ret {
        #[allow(clippy::expect_used)]
        arg.downcast_mut::<T>()
            .expect("Caller must verify the type is correct")
    }
}
