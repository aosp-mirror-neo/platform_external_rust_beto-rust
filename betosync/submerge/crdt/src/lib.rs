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

//! Implementation of state-based CRDTs.
//!
//! See documentation for [`CrdtState`] and the individual modules for more.

#![cfg_attr(docsrs, feature(doc_auto_cfg))]

use distributed_time::{
    vector_clock::VectorClock, DistributedClock, TimestampOverflow, WallTimestampProvider,
};
use lww_crdt_container::CrdtUnion;
use thiserror::Error;

pub mod lww_crdt_container;
pub mod lww_map;
pub mod register;
pub mod set;
pub mod typed_crdt;
pub mod vector_data;

#[cfg(any(test, feature = "checker"))]
#[allow(clippy::indexing_slicing, clippy::panic)]
pub mod checker;
pub mod delta;
mod utils;

/// Error returned when a CRDT merge fails.
///
/// When a merge error occurs, the CRDTs cannot be combined, and data convergence is not guaranteed.
#[derive(Debug, Error, PartialEq, Eq, Clone, Copy)]
pub enum MergeError {
    /// Merge failed because the types are incompatible.
    #[error("incompatible types")]
    IncompatibleType,
    /// Merge failed because the values are inconsistent.
    #[error("inconsistent values")]
    InconsistentValue,
}

/// A CRDT state that is mergeable with instance. The [`merge`][CrdtState::merge] operation must be:
///
/// * associative: `merge(merge(a, b)?, c)? == merge(a, merge(b, c)?)?`, and
/// * commutative: `merge(a, b)? == merge(b, a)?`, and
/// * idempotent: `merge(a, a)? == a`
///
/// These properties can be verified using
/// [`MergeInvariantTest`][crate::checker::MergeInvariantTest]. Any implementations of `CrdtState`
/// should have a corresponding `MergeInvariantTest` to verify its correctness.
///
/// Violating any of these properties is a logic error. The behavior resulting from a logic error is
/// not specified, but `unsafe` users of this trait MUST NOT rely on the correctness of these
/// methods for memory safety (since this is not an `unsafe trait`).
///
/// See also:
/// * [A comprehensive study of Convergent and Commutative Replicated Data
///   Types](https://inria.hal.science/inria-00555588/document)
pub trait CrdtState: Sized {
    /// Merges two CRDT states of the same type. See the trait documentation for details.
    fn merge(a: &Self, b: &Self) -> Result<Self, MergeError>;

    /// Return whether a given collection of CRDTs are valid.
    ///
    /// The default implementation returns true, meaning that any instances accepted by the type
    /// system can be merged with any other instance. If there are additional requirements, like if
    /// associated timestamps need to be globally unique, implementations can override this to
    /// return false in cases where different instances conflict with each other (see
    /// [`is_uniq`][crate::checker::utils::IterExt::is_uniq] and
    /// [`iter_pairs`][crate::checker::utils::IterExt::iter_pairs_unordered] for helper functions
    /// that can help implement this).
    ///
    /// Implementations must make sure these invalid combinations cannot be generated from mutations
    /// on the same ancestor state. [`Simulation`][crate::checker::simulation::Simulation] can be
    /// used to verify this.
    ///
    /// Requires the feature _`checker`_.
    #[cfg(any(test, feature = "checker"))]
    fn is_valid_collection<'a>(_collection: impl IntoIterator<Item = &'a Self>) -> bool
    where
        Self: 'a,
    {
        true
    }
}

/// Represents a CRDT that has a plain representation. A plain representation is the data in the
/// CRDT without the metadata (e.g. the timestamps used for Last-Writer-Wins).
///
/// A plain representation can be passed to the application for display and for modification. A
/// modified version of the plain representation can be applied to the original CRDT snapshot (using
/// `apply_changes`) to create a new, updated CRDT.
///
/// ## Performance warning
/// Because the plain representation has to recalculate the diff and "replay" the operations
/// performed, its performance can be much slower compared to the dedicated methods, especially on
/// large collections. This API is best used for prototyping, and should be avoided in production.
pub trait HasPlainRepresentation<N: Ord>: CrdtUnion<N>
where
    for<'d> Self::DeltaRef<'d>: ToPlain<Plain = Self::Plain>,
    for<'d> Self::DeltaMut<'d>: ApplyChanges<N, Plain = Self::Plain, Error = Self::Error>,
{
    /// The type for the plain representation of this CRDT.
    type Plain;
    /// Error type returned when applying changes or initializing the CRDT from a plain
    /// representation.
    type Error;

    /// Initializes a CRDT from the plain representation. If the CRDT implements [`Default`], the
    /// result should be the same as `Crdt::default().apply_changes(id, ctx)`.
    fn init_from_plain(
        ctx: &impl UpdateContext<N>,
        plain: Self::Plain,
    ) -> Result<Self, Self::Error>;
}

/// A type that has a plain, non-CRDT representation.
///
/// This plain representation can be thought of as the CRDT type without the metadata for
/// consistency. For example, for a [`set::Set<E>`], the plain type is simply
/// [`HashSet<E>`][std::collections::HashSet].
///
/// See [`HasPlainRepresentation`].
pub trait ToPlain {
    /// The plain representation that this type can convert into.
    type Plain;

    /// Convert this CRDT into its plain representation.
    fn to_plain(&self) -> Self::Plain;
}

/// Implemented by mutable delta CRDTs, this trait indicates that a type can apply changes from a
/// given plain representation.
///
/// See [`HasPlainRepresentation`].
pub trait ApplyChanges<N> {
    /// The plain representation that can be applied by this type.
    type Plain;
    /// The error that may be returned if applying a change failed.
    type Error;

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
        plain: Self::Plain,
    ) -> Result<bool, Self::Error>;
}

/// An update context for CRDTs. Many CRDT mutations require a context such as the updater's node ID
/// or the current wall clock time.
pub trait UpdateContext<N> {
    /// The provider for wall-clock timestamps.
    type WallClock: WallTimestampProvider;

    /// Get the updater associated with this update context, i.e. the node this code is running on.
    fn updater(&self) -> &N;
    /// Get the timestamp provider from this context.
    fn timestamp_provider(&self) -> &Self::WallClock;

    /// The current version of the document this CRDT is a part of.
    ///
    /// The CRDT implementations assume that this context is
    /// monotonically increasing, meaning that the version may be copied into the CRDT tree, and any
    /// later operations' context must return a version newer than or equal to the copied version
    /// value.
    fn version(&self) -> &VectorClock<N>
    where
        N: Ord;

    /// The next version of the document after the pending changes are committed.
    fn next_version(&self) -> Result<VectorClock<N>, TimestampOverflow>
    where
        N: Ord + Clone,
    {
        self.version().increment(self.updater())
    }
}

/// A type supporting content equality check besides the standard PartialEq check. This is useful if
/// a type contains both payload and metadata, in which case PartialEq compares both, while
/// ContentEq only compares the payload.
///
/// This corresponds to [equivalence relations](https://en.wikipedia.org/wiki/Equivalence_relation).
/// Implementations must make sure that the relation is symmetric, transitive, and reflexive.
pub trait ContentEq<Rhs: ?Sized = Self> {
    /// Perform a content equality check.
    fn content_eq(&self, other: &Rhs) -> bool;
}
