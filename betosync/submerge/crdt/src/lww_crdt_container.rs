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

//! A last-writer-wins container that is useful for holding a union of different CRDTs.
//!
//! See the docs for [`LwwCrdtContainer`].

use std::{cmp::Ordering, fmt::Debug};

use crate::{
    delta::{AsDeltaMut, AsDeltaRef, DeltaMut, DeltaRef},
    ApplyChanges, ContentEq, CrdtState, HasPlainRepresentation, ToPlain, UpdateContext,
};
use arbitrary::Arbitrary;
use derive_where::derive_where;
use distributed_time::{
    vector_clock::VectorClock, DistributedClock, TimestampOverflow, TotalTimestamp,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error returned when a [`CrdtUnion`] cannot be merged.
#[derive(Debug)]
pub struct IncompatibleType;

/// A [`CrdtUnion`] is a tagged union of [`CrdtState`]s, which can be implemented by a Rust `enum`
/// or a `dyn` type.
///
/// This type should implement [`TryAsChild`] and `From<Child>` for each of the variants it can
/// contain. The implementation of [`try_merge`][CrdtUnion::try_merge] delegate to the child's
/// [`CrdtState::merge`], or return [`IncompatibleType`] if the two given unions are not of the same
/// type. The merge must follow the same properties (Associative, Commutative, Idempotent) as
/// defined in [`CrdtState`].
///
/// This is primarily designed for use with [`LwwCrdtContainer`], which delegates the merging to the
/// child value iff the timestamps are equal.
pub trait CrdtUnion<N: Ord>: Sized {
    /// The read-only reference for the delta type of this CRDT union. This should be a union (enum
    /// or dyn-type) that has the same variants, such that for each variant `Variant(T)` in the
    /// union, there is a corresponding `Variant(DeltaRef<T>)` in the ref type.
    type DeltaRef<'d>
    where
        Self: 'd;

    /// The mutable reference for the delta type of this CRDT union. This should be a union (enum
    /// or dyn-type) that has the same variants, such that for each variant `Variant(T)` in the
    /// union, there is a corresponding `Variant(DeltaMut<T>)` in the mut type.
    type DeltaMut<'d>
    where
        Self: 'd;

    /// Merges two CRDT unions.
    ///
    /// If `a` and `b` are of the same variant, it should return
    /// `Self::from(CrdtState::merge(a.as_child(), b.as_child()))`. Otherwise, it should return
    /// `Err(IncompatibleType)`.
    fn try_merge(a: &Self, b: &Self) -> Result<Self, IncompatibleType>;

    /// Create a new read-only delta reference from the given `base` and `delta` components.
    fn create_ref<'d>(
        base: Option<&'d Self>,
        delta: Option<&'d Self>,
    ) -> Option<Self::DeltaRef<'d>>;

    /// Create a new mutable delta reference from the given `base` and `delta` components.
    fn create_mut<'d>(base: Option<&'d Self>, delta: &'d mut Self) -> Option<Self::DeltaMut<'d>>;

    /// Create a default (i.e. bottom state) for this union that has the same variant as `example`.
    ///
    /// For example, if `example` is `VariantN(TypeN { .. })`, then this should return
    /// `VariantN(TypeN::default())`.
    fn create_matching_default(example: &Self) -> Self;

    /// Calculates the delta needed to update from the given `base_version`.
    ///
    /// The `base_version` must be a version vector from a replica of the same document. The
    /// returned document can be applied using [`CrdtState::merge`] by supplying a base
    /// that is at least as new as `base_version`.
    ///
    /// Note that the version is not tracked in this crate. Users of this crate must track the
    /// versioning information externally.
    fn calculate_delta(&self, base_version: &VectorClock<N>) -> Self;
}

/// A trait for fallibly converting a [`CrdtUnion`] into its child type.
///
/// Usage of this type is similar to [`AsRef`] and [`AsMut`], with the additional requirement that
/// the container is a [`CrdtUnion`] and the contained child must be a [`CrdtState`]. See the
/// documentation for [`CrdtUnion`] for more.
pub trait TryAsChild<C, N: Ord>: From<C> + CrdtUnion<N> {
    /// Converts this union into a child CRDT state reference.
    fn try_as_child_ref(this: Self::DeltaRef<'_>) -> Option<DeltaRef<'_, C>>;
    /// Converts this union into a child CRDT state mutable reference.
    fn try_as_child_mut(this: Self::DeltaMut<'_>) -> Option<DeltaMut<'_, C>>;
}

/// A error returned by [`LwwCrdtContainerWrite::apply_changes`].
#[derive(Debug, PartialEq, Eq, Error)]
pub enum LwwCrdtContainerApplyError<E> {
    /// Merge failed because the types are incompatible.
    IncompatibleType,
    /// Merge failed because the timestamp overflowed the maximum value.
    TimestampOverflow,
    /// An error was returned by the child CRDT's `apply_changes`.
    ChildError(#[from] E),
}

/// A last-writer-wins container that contains a [`CrdtUnion`] as its value.
///
/// This container consists of two layers:
/// * `LwwCrdtContainer` contains a `V: CrdtUnion`. Modification of this layer is done through
///   [`set_value`][LwwCrdtContainerWrite::set_value], which follows last-writer-wins semantics.
/// * `V` in turn can switch between multiple types `Child: CrdtState` where `V: TryAsChild<Child>`.
///   Modification of this layer is done through
///   [`get_child_mut`][LwwCrdtContainerWrite::get_child_mut], which mutates without updating the
///   timestamp, and merges according to the semantics of the `Child` type.
///
/// The result is that you can have dynamically typed CRDTs at runtime (via enum or dyn types),
/// where the type information (i.e. the schema) merges by last writer wins, and the data merges
/// according to the schema defined type.
///
/// The merge operation is defined as follows:
/// * If the timestamps are not equal, the value associated with the larger timestamp is taken. The
///   value associated with the smaller timestamp is discarded, and NOT merged even if it is the
///   same type is the newer value.
/// * If the timestamps are equal, the two values are merged using the
///   [`try_merge`][CrdtUnion::try_merge] function, and is expected to succeed because `try_merge`
///   requires the implementation to delegate to the child variant, and the API surface of this
///   container does not allow changing the variant of the [`CrdtUnion`] without updating the
///   timestamp.
///
/// # Param
/// * `V` — The [`CrdtUnion`] type contained in this container.
/// * `T` — A total ordered timestamp use for last-writer-wins comparison. This timestamp must be
/// monotonically increasing, totally ordered, and globally unique. Typical usage should consider
/// using [`CompoundTimestamp`][distributed_time::compound_timestamp::CompoundTimestamp].
///
/// # Implementations
/// * See [`LwwCrdtContainerRead`] for methods on read-only references of [`LwwCrdtContainer`].
/// * See [`LwwCrdtContainerWrite`] for methods on mutable references of [`LwwCrdtContainer`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Arbitrary)]
#[derive_where(Default)]
pub struct LwwCrdtContainer<V, T, N: Ord> {
    value: Option<LwwCrdtContainerInner<V, T>>,
    subtree_version: VectorClock<N>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Arbitrary)]
#[derive_where(Default; T)]
struct LwwCrdtContainerInner<V, T> {
    value: Option<V>,
    timestamp: T,
}

impl<V, T, N: Ord> LwwCrdtContainer<V, T, N> {
    fn timestamp(&self) -> Option<&T> {
        self.value.as_ref().map(|inner| &inner.timestamp)
    }

    fn value(&self) -> Option<&V> {
        self.value.as_ref().and_then(|inner| inner.value.as_ref())
    }

    fn value_mut(&mut self) -> Option<&mut V> {
        self.value.as_mut().and_then(|inner| inner.value.as_mut())
    }

    /// The version of the entire subtree under this node.
    ///
    /// When a _subtree version updating operation_ is performed, this version is updated to
    /// [`UpdateContext::next_version`]. It represents the version of the document when changes were
    /// made to any nodes in the subtree, but may be updated conservatively even when the contents
    /// are unchanged. In the current implementation, this is conservatively updated whenever a
    /// mutable reference to the underlying value is returned.
    ///
    /// This version is used in [`LwwCrdtContainer::calculate_delta`] to selectively traverse down
    /// the tree and find all of the nodes that has updates not contained in a given version.
    pub fn subtree_version(&self) -> &VectorClock<N> {
        &self.subtree_version
    }

    /// Calculates the delta needed to update from the given `base_version`.
    ///
    /// The `base_version` must be a version vector from a replica of the same document. The
    /// returned document can be applied using [`CrdtState::merge`] by supplying a base
    /// that is at least as new as `base_version`.
    ///
    /// Note that the version is not tracked in this crate. Users of this crate must track the
    /// versioning information externally.
    pub fn calculate_delta(&self, base_version: &VectorClock<N>) -> Self
    where
        V: CrdtUnion<N>,
        N: Ord + Clone,
        T: Clone,
    {
        if base_version >= &self.subtree_version {
            // No new updates needed because the base is new enough. Just return the default which
            // will not result in any changes when merged.
            return Self::default();
        }
        // Otherwise, the base is older or concurrent, calculate the delta
        match &self.value {
            Some(inner) => match &inner.value {
                Some(value) => Self {
                    value: Some(LwwCrdtContainerInner {
                        value: Some(value.calculate_delta(base_version)),
                        timestamp: inner.timestamp.clone(),
                    }),
                    subtree_version: self.subtree_version.clone(),
                },
                None => Self {
                    value: Some(LwwCrdtContainerInner {
                        value: None,
                        timestamp: inner.timestamp.clone(),
                    }),
                    subtree_version: self.subtree_version.clone(),
                },
            },
            None => Self {
                value: None,
                subtree_version: self.subtree_version.clone(),
            },
        }
    }
}

/// Read-only operations for a CRDT container.
///
/// ## See also
/// * See [`LwwCrdtContainer`] for a description of the CRDT container type.
/// * See [`LwwCrdtContainerWrite`] for mutating operations on CRDT containers.
pub trait LwwCrdtContainerRead<V, T, N: Ord>: AsDeltaRef<LwwCrdtContainer<V, T, N>>
where
    V: 'static,
    T: 'static,
    N: 'static,
{
    /// Get the timestamp associated with the value in this container.
    fn timestamp(&self) -> Option<&T>
    where
        T: Ord,
    {
        let delta_timestamp = self.delta().and_then(|delta| delta.timestamp());
        let base_timestamp = self.base().and_then(|base| base.timestamp());
        delta_timestamp.max(base_timestamp)
    }

    /// Get the value in this container as a read-only reference.
    fn get_value(&self) -> Option<V::DeltaRef<'_>>
    where
        T: Ord,
        V: CrdtUnion<N>,
    {
        self.as_ref().into_value()
    }

    /// Get a reference to the child from this container, or `None` if the contained CRDT value
    /// cannot be converted to `Child`.
    ///
    /// A "child" is the type contained in one of the variants in the [`CrdtUnion`] (`V`).
    fn get_child<Child>(&self) -> Option<DeltaRef<'_, Child>>
    where
        V: TryAsChild<Child, N>,
        T: Ord,
    {
        self.get_value().and_then(V::try_as_child_ref)
    }

    /// Copies the data without the associated metadata and returns the plain type.
    ///
    /// See also: [`crate::HasPlainRepresentation`].
    fn to_plain(&self) -> Option<V::Plain>
    where
        V: CrdtUnion<N> + HasPlainRepresentation<N> + Clone,
        T: TotalTimestamp<NodeId = N> + Clone,
        // Bounds implied by V: HasPlainRepresentation below
        for<'d> V::DeltaRef<'d>: ToPlain<Plain = V::Plain>,
        for<'d> V::DeltaMut<'d>: ApplyChanges<N, Plain = V::Plain, Error = V::Error>,
    {
        self.get_value().map(|v| v.to_plain())
    }
}

impl<'d, V, T, N: Ord> DeltaRef<'d, LwwCrdtContainer<V, T, N>> {
    /// Convert this into a read-only reference to the value in this container.
    ///
    /// Returns `None` if the value exists in the base but not in the delta.
    pub fn into_value(self) -> Option<V::DeltaRef<'d>>
    where
        T: Ord,
        V: CrdtUnion<N>,
    {
        match (self.base, self.delta) {
            (None, None) => None,
            (None, Some(delta)) => V::create_ref(None, delta.value()),
            (Some(base), None) => V::create_ref(base.value(), None),
            (Some(base), Some(delta)) => match delta.timestamp().cmp(&base.timestamp()) {
                Ordering::Less => V::create_ref(base.value(), None),
                Ordering::Equal => V::create_ref(base.value(), delta.value()),
                Ordering::Greater => V::create_ref(None, delta.value()),
            },
        }
    }

    /// Convert this into a read-only reference to the child.
    ///
    /// A "child" is the type contained in one of the variants in the [`CrdtUnion`] (`V`).
    ///
    /// Returns `None` if the contained value cannot be converted into the `Child` type.
    pub fn into_child<Child>(self) -> Option<DeltaRef<'d, Child>>
    where
        V: TryAsChild<Child, N>,
        T: Ord,
    {
        self.into_value().and_then(V::try_as_child_ref)
    }
}

/// Implementation for [`LwwCrdtContainerWrite::set_value`] and
/// [`LwwCrdtContainerWrite::remove_value`]. I wish traits can have private functions.
fn set_value_impl<V, T, N, W>(
    this: &mut W,
    ctx: &impl UpdateContext<N>,
    value: Option<V>,
) -> Result<(), TimestampOverflow>
where
    V: 'static,
    T: TotalTimestamp<NodeId = N> + 'static,
    N: Ord + Clone + 'static,
    W: LwwCrdtContainerWrite<V, T, N> + ?Sized,
{
    let timestamp = T::increment_or_new(this.timestamp(), ctx.updater(), ctx.timestamp_provider())?;
    let delta_mut = this.delta_mut();
    let next_version = ctx.next_version()?;
    // Violation of this assertion is a fault in the update context. It means that version we
    // get from `ctx` is not monotonically increasing. Perhaps `ctx` is not the context
    // associated with this document?
    debug_assert!(next_version >= delta_mut.subtree_version);
    *delta_mut = LwwCrdtContainer {
        value: Some(LwwCrdtContainerInner { value, timestamp }),
        subtree_version: next_version,
    };
    Ok(())
}

/// Mutating operations for a CRDT container.
///
/// ## See also
/// * See [`LwwCrdtContainer`] for a description of the CRDT container type.
/// * See [`LwwCrdtContainerRead`] for read-only operations on CRDT containers.
pub trait LwwCrdtContainerWrite<V, T, N>:
    LwwCrdtContainerRead<V, T, N> + AsDeltaMut<LwwCrdtContainer<V, T, N>>
where
    V: 'static,
    T: 'static,
    N: Ord + Clone + 'static,
{
    /// Set this CRDT container to a new value. This updates the associated timestamp, and will
    /// always override other values with smaller timestamps without invoking
    /// [`V::try_merge`][CrdtUnion::try_merge]. If merging the child state is
    /// desired, use [`get_child_mut`][Self::get_child_mut] instead.
    fn set_value(&mut self, ctx: &impl UpdateContext<N>, value: V) -> Result<(), TimestampOverflow>
    where
        T: TotalTimestamp<NodeId = N>,
    {
        set_value_impl(self, ctx, Some(value))
    }

    /// Set the value in this container to `None`, updating the associated timestamp in the
    /// process.
    fn remove_value(&mut self, ctx: &impl UpdateContext<N>) -> Result<(), TimestampOverflow>
    where
        T: TotalTimestamp<NodeId = N>,
    {
        set_value_impl(self, ctx, None)
    }

    /// Get a mutable reference to the underlying union value, or `None` if the contained CRDT value
    /// does not exist.
    ///
    /// This operation does not increment the timestamp associated with the value. Any mutations
    /// made to the returned value will be merged according to the child's merge function.
    ///
    /// This is a _subtree version updating operation_ (See
    /// [`subtree_version`](LwwCrdtContainer::subtree_version)).
    fn get_value_mut(
        &mut self,
        ctx: &impl UpdateContext<N>,
    ) -> Result<Option<V::DeltaMut<'_>>, TimestampOverflow>
    where
        V: CrdtUnion<N>,
        T: Ord + Clone,
    {
        self.as_mut().into_value_mut(ctx)
    }

    /// Get a mutable reference to the child CRDT from this container, or `None` if the contained
    /// CRDT value cannot be converted to `Child`.
    ///
    /// This operation does not increment the timestamp associated with the value. Any mutations
    /// made to the returned value will be merged according to the child's merge function.
    ///
    /// This is a _subtree version updating operation_ (See
    /// [`subtree_version`](LwwCrdtContainer::subtree_version)).
    fn get_child_mut<Child>(
        &mut self,
        ctx: &impl UpdateContext<N>,
    ) -> Result<Option<DeltaMut<'_, Child>>, TimestampOverflow>
    where
        V: TryAsChild<Child, N>,
        T: Ord + Clone,
    {
        self.as_mut().into_child_mut(ctx)
    }

    /// Get the child CRDT from this container, or set it to `Child::default()`. Returns a mutable
    /// reference to the existing or newly added value, or `None` if the existing CRDT value cannot
    /// be converted to `Child`.
    ///
    /// This operation increments the timestamp associated with the value only if it was initially
    /// empty as was initialized with the default. Any subsequent changes made to the returned value
    /// does not increment the timestamp and will be merged according to the child's merge function.
    ///
    /// This is a _subtree version updating operation_ (See
    /// [`subtree_version`](LwwCrdtContainer::subtree_version)).
    fn get_child_or_default<Child>(
        &mut self,
        ctx: &impl UpdateContext<N>,
    ) -> Result<Option<DeltaMut<'_, Child>>, TimestampOverflow>
    where
        Child: Default,
        V: TryAsChild<Child, N>,
        T: TotalTimestamp<NodeId = N> + Clone,
    {
        self.as_mut().into_child_or_default(ctx)
    }

    /// Apply the changes in the plain representation into this CRDT. This assumes that all changes
    /// in the plain representation were made by the calling node, updating the associated CRDT
    /// metadata in the process.
    ///
    /// After applying the changes, [`to_plain`][ToPlain::to_plain] should return the same value as
    /// `plain`.
    ///
    /// # Returns
    ///
    /// `true` if applying the change results in an update in the CRDT state, or false if Returns
    /// true if applying the change results in an update in the CRDT state, or false if `plain` is
    /// the same as [`to_plain`][ToPlain::to_plain] to begin with. This can be used to avoid sending
    /// unnecessary update messages over the network, or writing changes to disk unnecessarily.
    fn apply_changes(
        &mut self,
        ctx: &impl UpdateContext<N>,
        plain: Option<V::Plain>,
    ) -> Result<bool, LwwCrdtContainerApplyError<V::Error>>
    where
        V: CrdtUnion<N> + HasPlainRepresentation<N> + Clone,
        for<'d> V::DeltaRef<'d>: ToPlain<Plain = V::Plain>,
        for<'d> V::DeltaMut<'d>: ApplyChanges<N, Plain = V::Plain, Error = V::Error>,
        T: TotalTimestamp<NodeId = N> + Clone,
    {
        let value = self
            .get_value_mut(ctx)
            .map_err(|_| LwwCrdtContainerApplyError::TimestampOverflow)?;
        let new_value = match (value, plain) {
            (None, None) => return Ok(false),
            (Some(_), None) => None,
            (None, Some(v)) => Some(V::init_from_plain(ctx, v)?),
            (Some(mut crdt), Some(plain)) => return Ok(crdt.apply_changes(ctx, plain)?),
        };
        set_value_impl(self, ctx, new_value)
            .map_err(|_| LwwCrdtContainerApplyError::TimestampOverflow)?;
        Ok(true)
    }
}

impl<'d, V, T, N: Ord> DeltaMut<'d, LwwCrdtContainer<V, T, N>> {
    /// Convert this into a mutable reference to the value in this container.
    ///
    /// If the value exists in the base but not in the delta, the value in the delta is initialized
    /// to the default value, so the mutations in the returned value are tracked.
    ///
    /// This is a _subtree version updating operation_ (See
    /// [`subtree_version`](LwwCrdtContainer::subtree_version)).
    pub fn into_value_mut(
        self,
        ctx: &impl UpdateContext<N>,
    ) -> Result<Option<V::DeltaMut<'d>>, TimestampOverflow>
    where
        V: CrdtUnion<N>,
        T: Clone + Ord,
        N: Clone,
    {
        let next_version = ctx.next_version()?;
        // Violation of this assertion is a fault in the update context. It means that version we
        // get from `ctx` is not monotonically increasing. Perhaps `ctx` is not the context
        // associated with this document?
        debug_assert!(next_version >= self.delta.subtree_version,);
        self.delta.subtree_version = next_version;
        let base_timestamp = self.base.and_then(|b| b.timestamp());
        match self.delta.timestamp().cmp(&base_timestamp) {
            Ordering::Less => {
                // Assign a value to delta if base exists, so it modifications to the value can be
                // recorded. This is hoisted up from the match expression below to satisfy the
                // borrow checker.
                let base_value = self.base.and_then(|b| b.value());
                self.delta.value = Some(LwwCrdtContainerInner {
                    value: base_value.map(V::create_matching_default),
                    timestamp: base_timestamp
                        .expect("Must not be None since base_timestamp > delta_timestamp")
                        .clone(),
                });
                if let Some(delta_value) = self.delta.value_mut() {
                    Ok(V::create_mut(base_value, delta_value))
                } else {
                    Ok(None)
                }
            }
            Ordering::Equal => {
                let base = self.base.and_then(|b| b.value());
                let delta = self.delta.value_mut();
                match (base, delta) {
                    (None, None) => Ok(None),
                    (Some(_), None) => unreachable!(),
                    (_, Some(delta_value)) => Ok(V::create_mut(base, delta_value)),
                }
            }
            Ordering::Greater => Ok(self.delta.value_mut().and_then(|v| V::create_mut(None, v))),
        }
    }

    /// Convert this into a mutable reference to the child.
    ///
    /// A "child" is the type contained in one of the variants in the [`CrdtUnion`] (`V`).
    ///
    /// Returns `Ok(None)` if the contained value cannot be converted into the `Child` type.
    ///
    /// This is a _subtree version updating operation_ (See
    /// [`subtree_version`](LwwCrdtContainer::subtree_version)).
    pub fn into_child_mut<Child>(
        self,
        ctx: &impl UpdateContext<N>,
    ) -> Result<Option<DeltaMut<'d, Child>>, TimestampOverflow>
    where
        V: TryAsChild<Child, N>,
        T: Clone + Ord,
        N: Clone,
    {
        Ok(self.into_value_mut(ctx)?.and_then(V::try_as_child_mut))
    }

    /// Convert this into a mutable reference to the child, initializing it to default if needed.
    ///
    /// A "child" is the type contained in one of the variants in the [`CrdtUnion`] (`V`).
    ///
    /// If the current value in the container is none, it will be initialized to `Child::default()`,
    /// and a reference to the newly initialized value will be returned. If there is an existing
    /// child in this container, but it is not convertible to `Child`, then `None` is returned.
    ///
    /// This is a _subtree version updating operation_ (See
    /// [`subtree_version`](LwwCrdtContainer::subtree_version)).
    pub fn into_child_or_default<Child>(
        mut self,
        ctx: &impl UpdateContext<N>,
    ) -> Result<Option<DeltaMut<'d, Child>>, TimestampOverflow>
    where
        Child: Default,
        V: TryAsChild<Child, N> + 'static,
        T: TotalTimestamp<NodeId = N> + Clone + 'static,
        N: Clone + 'static,
    {
        if self.get_value().is_none() {
            let _ = self.set_value(ctx, Child::default().into()).ok();
        }
        self.into_child_mut(ctx)
    }
}

impl<V, T, N> CrdtState for LwwCrdtContainer<V, T, N>
where
    V: CrdtUnion<N> + Clone,
    T: TotalTimestamp + Clone,
    N: Ord + Clone,
{
    fn merge(a: &Self, b: &Self) -> Self {
        fn merge_inner<V, T, N: Ord>(
            a: &LwwCrdtContainerInner<V, T>,
            b: &LwwCrdtContainerInner<V, T>,
        ) -> LwwCrdtContainerInner<V, T>
        where
            V: CrdtUnion<N> + Clone,
            T: TotalTimestamp + Clone,
        {
            match a.timestamp.cmp(&b.timestamp) {
                Ordering::Equal => {
                    match (&a.value, &b.value) {
                        (Some(value_a), Some(value_b)) => LwwCrdtContainerInner {
                            value: Some(
                                CrdtUnion::try_merge(value_a, value_b)
                                    // This container does not allow updating the value without also
                                    // updating the timestamp outside of mutating a child CRDT, and
                                    // `try_merge` is required to delegate to the child CRDT's
                                    // `merge` which is infallible.
                                    .expect("Values with the same timestamp should be mergeable."),
                            ),
                            timestamp: a.timestamp.clone(),
                        },
                        (Some(_), None) => a.clone(),
                        (None, Some(_)) => b.clone(),
                        (None, None) => a.clone(),
                    }
                }
                Ordering::Less => b.clone(),
                Ordering::Greater => a.clone(),
            }
        }

        LwwCrdtContainer {
            value: match (&a.value, &b.value) {
                (Some(inner_a), Some(inner_b)) => Some(merge_inner(inner_a, inner_b)),
                (Some(inner), None) | (None, Some(inner)) => Some(inner.clone()),
                (None, None) => None,
            },
            subtree_version: VectorClock::least_upper_bound(&a.subtree_version, &b.subtree_version),
        }
    }

    #[cfg(any(test, feature = "checker"))]
    fn is_valid_collection<'a>(collection: impl IntoIterator<Item = &'a Self>) -> bool
    where
        Self: 'a,
    {
        use crate::checker::utils::IterExt as _;

        collection
            .into_iter()
            .map(|s| s.timestamp())
            .by_ref()
            .is_uniq()
    }
}

impl<V, T, N> ContentEq for LwwCrdtContainer<V, T, N>
where
    V: CrdtUnion<N> + ContentEq,
    T: TotalTimestamp,
    N: Ord,
{
    fn content_eq(&self, other: &Self) -> bool {
        match (&self.value, &other.value) {
            (Some(self_inner), Some(other_inner)) => {
                if self_inner.timestamp != other_inner.timestamp {
                    return false;
                }
                match (&self_inner.value, &other_inner.value) {
                    (Some(self_crdt), Some(other_crdt)) => self_crdt.content_eq(other_crdt),
                    (None, None) => true,
                    _ => false,
                }
            }
            (None, None) => true,
            _ => false,
        }
    }
}

impl<V, T, N, C> LwwCrdtContainerRead<V, T, N> for C
where
    V: 'static,
    T: 'static,
    N: Ord + 'static,
    C: AsDeltaRef<LwwCrdtContainer<V, T, N>>,
{
}
impl<V, T, N, C> LwwCrdtContainerWrite<V, T, N> for C
where
    V: 'static,
    T: 'static,
    N: Ord + Clone + 'static,
    C: AsDeltaMut<LwwCrdtContainer<V, T, N>>,
{
}

#[cfg(feature = "proto")]
mod proto {
    use distributed_time::{
        hybrid_logical_clock::HybridLogicalTimestamp, vector_clock::VectorClock,
    };
    use submerge_internal_proto::{FromProto, FromProtoError, NodeMapping, ToProto};

    use crate::typed_crdt::{proto::TypedCrdtFromProtoError, TypedCrdt};

    use super::{LwwCrdtContainer, LwwCrdtContainerInner};

    impl FromProto
        for LwwCrdtContainer<TypedCrdt<Vec<u8>, String>, HybridLogicalTimestamp<String>, String>
    {
        type Proto = submerge_internal_proto::protos::submerge::SubmergeContainer;

        fn from_proto(proto: &Self::Proto, node_ids: &[String]) -> Result<Self, FromProtoError> {
            let value = match proto
                .value
                .as_ref()
                .map(|v| TypedCrdt::from_proto(v, node_ids))
            {
                Some(Ok(v)) => Some(v),
                Some(Err(TypedCrdtFromProtoError::FromProtoError(e))) => return Err(e),
                Some(Err(TypedCrdtFromProtoError::UnrecognizedType)) => {
                    // For forwards compatibility, ignore any unknown type and treat as if this node
                    // is empty.
                    return Ok(Self::default());
                }
                None => None,
            };
            Ok(Self {
                value: match proto.timestamp.as_ref() {
                    Some(t) => Some(LwwCrdtContainerInner {
                        value,
                        timestamp: HybridLogicalTimestamp::from_proto(
                            t,
                            proto.updater.ok_or(FromProtoError::MissingRequiredField)?,
                            node_ids,
                        )?,
                    }),
                    None => None,
                },
                subtree_version: VectorClock::from_proto(&proto.subtree_version, node_ids)?,
            })
        }
    }

    impl ToProto
        for LwwCrdtContainer<TypedCrdt<Vec<u8>, String>, HybridLogicalTimestamp<String>, String>
    {
        type Proto = submerge_internal_proto::protos::submerge::SubmergeContainer;

        fn to_proto(&self, node_ids: &mut NodeMapping<String>) -> Self::Proto {
            let hlc = self.timestamp();
            let (hlc_proto, updater) = hlc.map(|t| t.to_proto(node_ids)).unzip();
            submerge_internal_proto::protos::submerge::SubmergeContainer {
                updater,
                timestamp: hlc_proto.into(),
                subtree_version: Some(self.subtree_version().to_proto(node_ids)).into(),
                value: self.value().map(|v| v.to_proto(node_ids)),
                ..Default::default()
            }
        }
    }

    #[cfg(test)]
    #[derive_fuzztest::proptest]
    fn container_roundtrip(
        container: LwwCrdtContainer<
            TypedCrdt<Vec<u8>, String>,
            HybridLogicalTimestamp<String>,
            String,
        >,
    ) {
        let mut node_ids = NodeMapping::default();
        assert_eq!(
            LwwCrdtContainer::from_proto(&container.to_proto(&mut node_ids), &node_ids.into_vec())
                .unwrap(),
            container,
        );
    }
}

#[cfg(test)]
mod tests {
    use distributed_time::{
        hybrid_logical_clock::HybridLogicalTimestamp, vector_clock::VectorClock,
    };
    use serde_json::json;

    use crate::{
        checker::test_fakes::FakeContext,
        lww_crdt_container::{LwwCrdtContainerRead, LwwCrdtContainerWrite},
        register::{Register, RegisterRead, RegisterWrite},
        set::Set,
        typed_crdt::{TypedCrdt, TypedCrdtRef},
        vector_data::VectorData,
        CrdtState,
    };

    use super::LwwCrdtContainer;

    type TestContainer =
        LwwCrdtContainer<TypedCrdt<&'static str, u8>, HybridLogicalTimestamp<u8>, u8>;

    #[test]
    fn test_set_value() {
        let mut reg = Register::<_, _>::default();
        let ctx = FakeContext::new(0, 0_u8);
        reg.set(&ctx, "1234").unwrap();
        let mut container = TestContainer::default();
        container.set_value(&ctx, reg.into()).unwrap();

        assert_eq!(
            &"1234",
            container
                .get_child_mut::<Register<_, _>>(&ctx)
                .unwrap()
                .unwrap()
                .get()
                .unwrap()
        );
    }

    #[test]
    fn test_get_as_wrong_type() {
        let mut reg = Register::<_, VectorClock<u8>>::default();
        let ctx = FakeContext::new(0, 0_u8);
        reg.set(&ctx, "1234").unwrap();
        let mut container = TestContainer::default();
        container.set_value(&ctx, reg.into()).unwrap();

        assert!(container
            .get_child_mut::<Set<&'static str>>(&ctx)
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_merge() {
        let mut container1 = TestContainer::default();
        container1
            .set_value(&FakeContext::new(0, 0_u8), Register::default().into())
            .unwrap();
        let mut container2: LwwCrdtContainer<
            TypedCrdt<&'static str, u8>,
            HybridLogicalTimestamp<u8>,
            u8,
        > = LwwCrdtContainer::default();
        container2
            .set_value(&FakeContext::new(1, 2_u8), VectorData::default().into())
            .unwrap();

        let merged = CrdtState::merge(&container1, &container2);
        // container2 wins because its timestamp is larger
        assert!(matches!(
            merged.get_value(),
            Some(TypedCrdtRef::VectorData(_))
        ));
    }

    #[test]
    fn test_merge_causal() {
        let mut container1 = TestContainer::default();
        container1
            .set_value(&FakeContext::new(0, 2_u8), Register::default().into())
            .unwrap();

        let mut ctx2 = FakeContext::new(1, 0_u8);
        ctx2.version = VectorClock::new([(0, 1)]);
        let mut container2 = container1.clone();
        container2
            .set_value(&ctx2, VectorData::default().into())
            .unwrap();

        let merged = CrdtState::merge(&container1, &container2);
        // d2 wins even though its timestamp is smaller, because it has observed d1 (d2 is cloned
        // from d1)
        assert!(matches!(
            merged.get_value(),
            Some(TypedCrdtRef::VectorData(_))
        ));
    }

    #[test]
    fn test_get_mut() {
        let mut orig_reg = Register::<_, _>::default();
        let ctx = FakeContext::new(0, 0_u8);
        orig_reg.set(&ctx, "1234").unwrap();
        let mut container1 = TestContainer::default();
        container1.set_value(&ctx, orig_reg.into()).unwrap();

        let mut container2 = container1.clone();

        let _ = container2
            .get_child_mut::<Register<_, _>>(&ctx)
            .unwrap()
            .unwrap();
        // get_child_mut alone should not mutate the container
        assert_eq!(container1, container2);

        // container2 set reg = "5678"
        let mut child_reg2 = container2
            .get_child_mut::<Register<_, _>>(&ctx)
            .unwrap()
            .unwrap();
        child_reg2.set(&FakeContext::new(1, 12_u8), "5678").unwrap();

        // container1 set reg = "abcd"
        let mut child_reg1 = container1
            .get_child_mut::<Register<_, _>>(&ctx)
            .unwrap()
            .unwrap();
        child_reg1.set(&FakeContext::new(0, 10_u8), "abcd").unwrap();

        let merged = CrdtState::merge(&container1, &container2);
        let merged_child_reg = merged.get_child::<Register<_, _>>().unwrap();
        // Merge should be done by Register rules, which is to keep all conflicting values.
        // "5678" comes first because it is newer (timestamp value of 12)
        assert_eq!(merged_child_reg.get_all(), vec![&"5678", &"abcd"]);
    }

    #[test]
    fn json_encode() {
        let mut container =
            LwwCrdtContainer::<TypedCrdt<String, u16>, HybridLogicalTimestamp<u16>, u16>::default();
        let _ = container
            .get_child_or_default::<Register<String, VectorClock<u16>>>(&FakeContext::new(0, 0_u8));
        let json_value = serde_json::to_value(container).unwrap();
        let expected_json = json! ({
            "value": {
                "value": {
                    "Register": [],
                },
                "timestamp": {
                    "time": {
                        "logical_time": 0,
                        "causality": 0,
                    },
                    "updater": 0
                },
            },
            "subtree_version": {
                "0": 1
            },
        });

        assert_eq!(expected_json, json_value);
    }

    #[test]
    fn json_decode() {
        let json = json! ({
            "value": {
                "value": {
                    "Register": [
                        {
                            "value": "#0",
                            "timestamp": {
                                "distributed_clock": {},
                                "hybrid_logical_timestamp": {
                                    "logical_time": 1,
                                    "causality": 0,
                                },
                            }
                        },
                        {
                            "value": "#1",
                            "timestamp": {
                                "distributed_clock": {},
                                "hybrid_logical_timestamp": {
                                    "logical_time": 2,
                                    "causality": 0,
                                },
                            }
                        }
                    ],
                },
                "timestamp": {
                    "time": {
                        "logical_time": 1,
                        "causality": 1,
                    },
                    "updater": 0
                },
            },
            "subtree_version": {
                "0": 1
            },
        });
        let container = serde_json::from_value::<
            LwwCrdtContainer<TypedCrdt<String, u16>, HybridLogicalTimestamp<u16>, u16>,
        >(json)
        .unwrap();
        assert_eq!(
            container.get_child::<Register<_, _>>().unwrap().get_all(),
            vec!["#1", "#0"]
        );
    }
}
