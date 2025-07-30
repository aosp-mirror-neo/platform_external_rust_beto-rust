// Copyright 2024 Google LLC
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

//! Representation for delta CRDT types.
//!
//! A delta CRDT type consists of two components:
//! 1. the `base`, which is the read only part that this delta CRDT is building on top of, and
//! 2. the `delta`, which is a mutable part that contains the changes within this delta.
//!
//! For example, a delta CRDT can be created during a transaction, in which the delta component
//! keeps track of the mutations made within the transaction. This allows the resulting update
//! message generated for the network interface to only contain the latest changes.
//!
//! This module offers three containers for delta CRDTs:
//! * [`DeltaOwned`] owns its own delta component, and has a shared reference to the base component.
//! * [`DeltaMut`] has a mutable reference to the delta component, and has a shared reference to the
//!   base component.
//! * [`DeltaRef`] has shared references to both the delta and base components.
//!
//! [`CrdtState`] implementations and [`DeltaOwned`] can be converted into [`DeltaMut`] using
//! [`as_mut`][AsDeltaMut::as_mut], and can be converted into [`DeltaRef`] using
//! [`as_ref`][AsDeltaRef::as_ref].

use crate::CrdtState;

/// A read-only reference to a base and delta components to a CRDT.
///
/// This contains a reference to the `base` component, which is the read-only component representing
/// the state when this delta was created, and the `delta` component, where the mutations since the
/// creation of the CRDT are stored.
#[derive(Debug, PartialEq, Eq)]
pub struct DeltaRef<'d, T> {
    /// The base component, which can be `None` if there is no base.
    pub base: Option<&'d T>,
    /// The delta component.
    ///
    /// This can be `None` if delta component doesn't exist, for example when trying to get an
    /// element from a map that only exist in the base component.
    pub delta: Option<&'d T>,
}

impl<T> DeltaRef<'_, T> {
    /// Combines the base part and the delta part and return the resulting merged component.
    ///
    /// See [`CrdtState::merge`] for details on the merge operation.
    pub fn merge(&self) -> T
    where
        T: CrdtState + Clone + Default,
    {
        match (self.base, self.delta) {
            (None, None) => T::default(),
            (None, Some(delta)) => delta.clone(),
            (Some(base), None) => base.clone(),
            (Some(base), Some(delta)) => T::merge(base, delta),
        }
    }
}

impl<C> AsDeltaRef<C> for DeltaRef<'_, C> {
    fn base(&self) -> Option<&C> {
        self.base
    }

    fn delta(&self) -> Option<&C> {
        self.delta
    }
}

/// A mutable reference to a delta-CRDT.
///
/// Contains a reference to the `base` component, which is the read-only component representing the
/// state when this delta was created, and the `delta` component, where the mutations since the
/// creation of the CRDT are stored.
///
/// Semantically this delta type behaves as if it is `CrdtState::merge(base, delta)`.
#[derive(Debug)]
pub struct DeltaMut<'d, T> {
    /// The base component, which can be `None` if there is no base.
    pub base: Option<&'d T>,
    /// The delta component.
    ///
    /// Unlike [`DeltaRef`], this is not an `Option`. Instead, if the value doesn't exist, the
    /// parent container or collection should insert the default value such that any modifications
    /// to the subtree can be tracked.
    pub delta: &'d mut T,
}

impl<C> AsDeltaRef<C> for DeltaMut<'_, C> {
    fn base(&self) -> Option<&C> {
        self.base
    }

    fn delta(&self) -> Option<&C> {
        Some(self.delta)
    }
}

impl<C> AsDeltaMut<C> for DeltaMut<'_, C> {
    fn delta_mut(&mut self) -> &mut C {
        self.delta
    }

    fn as_mut(&mut self) -> DeltaMut<'_, C> {
        DeltaMut {
            base: self.base,
            delta: self.delta,
        }
    }
}

/// A CRDT structure where the delta-component is owned.
///
/// Contains a reference to the `base` component, which is the read-only component representing the
/// state when this delta was created, and the `delta` component, where the mutations since the
/// creation of the CRDT are stored.
#[derive(Debug)]
pub struct DeltaOwned<'b, T> {
    /// The base component.
    ///
    /// Unlike [`DeltaMut`] and [`DeltaRef`], this is not an `Option` because a `DeltaOwned` without
    /// a base is the same as just `T`.
    pub base: &'b T,
    /// The delta component.
    pub delta: T,
}

impl<'b, T> DeltaOwned<'b, T> {
    /// Creates a new owned delta from the given base.
    ///
    /// The delta component is initially empty, so the contents are semantically the same as the
    /// given `base`.
    pub fn new(base: &'b T) -> Self
    where
        T: Default,
    {
        Self {
            base,
            delta: Default::default(),
        }
    }

    /// Merges the base and the delta components of this.
    ///
    /// See [`CrdtState::merge`] for details on the merge operation.
    pub fn merge(&self) -> T
    where
        T: CrdtState,
    {
        T::merge(self.base, &self.delta)
    }

    /// Converts this into its delta components, discarding the reference to the
    /// base component.
    pub fn into_delta(self) -> T {
        self.delta
    }
}

impl<C: CrdtState> AsDeltaRef<C> for DeltaOwned<'_, C> {
    fn base(&self) -> Option<&C> {
        Some(self.base)
    }

    fn delta(&self) -> Option<&C> {
        Some(&self.delta)
    }
}

impl<C: CrdtState> AsDeltaMut<C> for DeltaOwned<'_, C> {
    fn delta_mut(&mut self) -> &mut C {
        &mut self.delta
    }

    fn as_mut(&mut self) -> DeltaMut<'_, C> {
        DeltaMut {
            base: Some(self.base),
            delta: &mut self.delta,
        }
    }
}

/// Something that can behave like a read-only reference to a delta CRDT.
pub trait AsDeltaRef<C> {
    /// The base component. This is `None` if there is no base, for example if the given type is
    /// just a CRDT component.
    fn base(&self) -> Option<&C>;

    /// The delta component. This is `None` if there is no delta, for example when a value is read
    /// from a map in which the entry only exists in the base component.
    fn delta(&self) -> Option<&C>;

    /// Returns a [`DeltaRef`] from this.
    fn as_ref(&self) -> DeltaRef<'_, C> {
        DeltaRef {
            base: self.base(),
            delta: self.delta(),
        }
    }
}

/// Something that can behave like a mutable reference to a delta CRDT.
pub trait AsDeltaMut<C>: AsDeltaRef<C> {
    /// The delta component.
    ///
    /// Unlike [`AsDeltaRef::delta`], this cannot be `None`. If the value does not exist yet it
    /// should be initialized with `Default::default()`.
    fn delta_mut(&mut self) -> &mut C;

    /// Returns a [`DeltaMut`] from this.
    fn as_mut(&mut self) -> DeltaMut<'_, C>;
}

/// A blanket implementation for any [`CrdtState`] to be treated as a delta, which has no base
/// component and the CRDT's state is treated as the delta.
impl<C: CrdtState> AsDeltaRef<C> for C {
    fn base(&self) -> Option<&C> {
        None
    }

    fn delta(&self) -> Option<&C> {
        Some(self)
    }
}

/// A blanket implementation for any [`CrdtState`] to be treated as a delta, which has no base
/// component and the CRDT's state is treated as the delta.
impl<C: CrdtState> AsDeltaMut<C> for C {
    fn delta_mut(&mut self) -> &mut C {
        self
    }

    fn as_mut(&mut self) -> DeltaMut<'_, C> {
        DeltaMut {
            base: None,
            delta: self,
        }
    }
}
