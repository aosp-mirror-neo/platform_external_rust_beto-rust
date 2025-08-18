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

//! A specific typed CRDT used to implement the data model defined by this crate.
//!
//! See the docs for [`TypedCrdt`] for details.

use std::{
    collections::{BTreeMap, HashSet},
    fmt::Debug,
    hash::{BuildHasher, Hash, RandomState},
};

use crate::{
    delta::{DeltaMut, DeltaRef},
    lww_crdt_container::TryAsChild,
    lww_map::{LwwMapRead, LwwMapWrite},
    register::{RegisterRead, RegisterWrite},
    set::{SetRead, SetWrite},
    vector_data::{VectorDataRead, VectorDataWrite},
    ApplyChanges, ContentEq, CrdtState, HasPlainRepresentation, ToPlain, UpdateContext,
};
use arbitrary::{Arbitrary, Unstructured};
use derive_where::derive_where;
use distributed_time::{hybrid_logical_clock::HybridLogicalTimestamp, vector_clock::VectorClock};
use serde::{Deserialize, Serialize};

use crate::{
    lww_crdt_container::{CrdtUnion, IncompatibleType, LwwCrdtContainerApplyError},
    lww_map::LwwMap,
    register::{Register, RegisterError},
    set::{GenerationExhausted, Set},
    vector_data::{VectorData, VectorDataError},
};

/// An enum of the CRDT types supported by this crate.
///
/// In this data model, the main structural component is the `Map`, which has a string key and a
/// recursive `TypedCrdt` as the value, creating a tree-like structure. The values contained in this
/// data model must be of type `V`; if dynamic typing is desired, users of this type must define
/// their own enum or `dyn` type to hold the type information and switch between different
/// underlying types.
#[derive(Debug, Serialize, Deserialize)]
#[derive_where(PartialEq, Eq; V, N)]
#[derive_where(Clone; S)]
#[serde(bound(
    serialize = "V: Serialize, N: Serialize",
    deserialize = "V: Deserialize<'de>, N: Deserialize<'de>"
))]
pub enum TypedCrdt<V, N, S = RandomState>
where
    V: Eq + Hash + Clone + 'static,
    N: Ord + Clone + 'static,
    S: BuildHasher + Default + 'static,
{
    /// Map is the main structural component of this data model. It recursively contains `TypedCrdt`
    /// as values, creating a tree-like structure. See [`LwwMap`][crate::lww_map::LwwMap].
    Map(LwwMap<String, TypedCrdt<V, N, S>, HybridLogicalTimestamp<N>, N>),
    /// Register contains a single atomically updated value. See
    /// [`Register`][crate::register::Register].
    Register(Register<V, VectorClock<N>>),
    /// A no-duplicate, unordered set of values. See [`Set`][crate::set::Set].
    Set(Set<V, S>),
    /// A vector of data where each node can only modify its own area. See
    /// [`VectorData`][crate::vector_data::VectorData].
    VectorData(VectorData<N, V>),
}

// Manual implementation of `Arbitrary` so it doesn't bound on `S: Arbitrary`.
impl<'a, V, N, S> Arbitrary<'a> for TypedCrdt<V, N, S>
where
    V: Eq + Hash + Clone + Arbitrary<'a> + 'static,
    N: Ord + Clone + Arbitrary<'a> + 'static,
    S: BuildHasher + Default + 'static,
{
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        match u.int_in_range(0..=3)? {
            0 => Ok(Self::Map(u.arbitrary()?)),
            1 => Ok(Self::Register(u.arbitrary()?)),
            2 => Ok(Self::Set(u.arbitrary()?)),
            3 => Ok(Self::VectorData(u.arbitrary()?)),
            _ => unreachable!(),
        }
    }
}

/// An enum type that has the same variants as [`TypedCrdt`] but contains [`DeltaRef`] as the values
/// contained.
#[derive(Debug)]
pub enum TypedCrdtRef<'d, V, N, S>
where
    V: Eq + Hash + Clone + 'static,
    N: Ord + Clone + 'static,
    S: BuildHasher + Default + 'static,
{
    /// Map is the main structural component of this data model. It recursively contains `TypedCrdt`
    /// as values, creating a tree-like structure. See [`LwwMap`].
    #[allow(clippy::type_complexity)]
    Map(DeltaRef<'d, LwwMap<String, TypedCrdt<V, N, S>, HybridLogicalTimestamp<N>, N>>),
    /// Register contains a single atomically updated value. See [`Register`].
    Register(DeltaRef<'d, Register<V, VectorClock<N>>>),
    /// A no-duplicate, unordered set of values. See [`Set`].
    Set(DeltaRef<'d, Set<V, S>>),
    /// A vector of data where each node can only modify its own area. See [`VectorData`].
    VectorData(DeltaRef<'d, VectorData<N, V>>),
}

/// An enum type that has the same variants as [`TypedCrdt`] but contains [`DeltaMut`] as the values
/// contained.
#[derive(Debug)]
pub enum TypedCrdtMut<'d, V, N, S = RandomState>
where
    V: Eq + Hash + Clone + 'static,
    N: Ord + Clone + 'static,
    S: BuildHasher + Default + 'static,
{
    /// Map is the main structural component of this data model. It recursively contains `TypedCrdt`
    /// as values, creating a tree-like structure. See [`LwwMap`].
    #[allow(clippy::type_complexity)]
    Map(DeltaMut<'d, LwwMap<String, TypedCrdt<V, N, S>, HybridLogicalTimestamp<N>, N>>),
    /// Register contains a single atomically updated value. See [`Register`].
    Register(DeltaMut<'d, Register<V, VectorClock<N>>>),
    /// A no-duplicate, unordered set of values. See [`Set`].
    Set(DeltaMut<'d, Set<V, S>>),
    /// A vector of data where each node can only modify its own area. See [`VectorData`].
    VectorData(DeltaMut<'d, VectorData<N, V>>),
}

impl<N, V, S> ToPlain for TypedCrdtRef<'_, V, N, S>
where
    V: Eq + Hash + Clone,
    N: Ord + Clone,
    S: BuildHasher + Default + Clone,
{
    type Plain = PlainTypes<N, V, S>;

    fn to_plain(&self) -> PlainTypes<N, V, S> {
        match self {
            TypedCrdtRef::Map(m) => PlainTypes::Map(m.to_plain()),
            TypedCrdtRef::Register(v) => PlainTypes::Register(v.to_plain()),
            TypedCrdtRef::Set(v) => PlainTypes::Set(v.to_plain()),
            TypedCrdtRef::VectorData(v) => PlainTypes::VectorData(v.to_plain()),
        }
    }
}

impl<N, V, S> ApplyChanges<N> for TypedCrdtMut<'_, V, N, S>
where
    V: Eq + Hash + Clone,
    N: Ord + Clone,
    S: BuildHasher + Default + Clone,
{
    type Plain = PlainTypes<N, V, S>;
    type Error = TypedError;

    fn apply_changes(
        &mut self,
        ctx: &impl UpdateContext<N>,
        plain: Self::Plain,
    ) -> Result<bool, Self::Error> {
        Ok(match (self, plain) {
            (TypedCrdtMut::Map(map_crdt), PlainTypes::Map(plain_map)) => {
                map_crdt.apply_changes(ctx, plain_map)?
            }
            (TypedCrdtMut::Register(reg_crdt), PlainTypes::Register(plain_reg)) => {
                reg_crdt.apply_changes(ctx, plain_reg)?
            }
            (TypedCrdtMut::Set(set_crdt), PlainTypes::Set(plain_set)) => {
                set_crdt.apply_changes(plain_set)?
            }
            (TypedCrdtMut::VectorData(vd_crdt), PlainTypes::VectorData(plain_vd)) => {
                vd_crdt.apply_changes(ctx, plain_vd)?
            }
            _ => return Err(TypedError::IncompatibleType),
        })
    }
}

macro_rules! impl_try_as_child {
    (impl<V, N, S> TryAsChild<$ty:ty, N> for TypedCrdt<V, N, S>::$variant:ident) => {
        impl<V, N, S> TryAsChild<$ty, N> for TypedCrdt<V, N, S>
        where
            V: Eq + Hash + Clone + 'static,
            N: Ord + Clone + 'static,
            S: BuildHasher + Default + Clone + 'static,
        {
            fn try_as_child_ref(
                this: Self::DeltaRef<'_>,
            ) -> Option<$crate::delta::DeltaRef<'_, $ty>> {
                match this {
                    TypedCrdtRef::$variant(v) => Some(v),
                    _ => None,
                }
            }

            fn try_as_child_mut(
                this: Self::DeltaMut<'_>,
            ) -> Option<$crate::delta::DeltaMut<'_, $ty>> {
                match this {
                    TypedCrdtMut::$variant(v) => Some(v),
                    _ => None,
                }
            }
        }
    };
}

impl_try_as_child! {
    impl<V, N, S> TryAsChild<
        LwwMap<String, TypedCrdt<V, N, S>, HybridLogicalTimestamp<N>, N>, N
    > for TypedCrdt<V, N, S>::Map
}
impl_try_as_child! {
    impl<V, N, S> TryAsChild<Register<V, VectorClock<N>>, N> for TypedCrdt<V, N, S>::Register
}
impl_try_as_child! { impl<V, N, S> TryAsChild<Set<V, S>, N> for TypedCrdt<V, N, S>::Set }
impl_try_as_child! { impl<V, N, S> TryAsChild<VectorData<N, V>, N> for TypedCrdt<V, N, S>::VectorData }

impl<V, N, S> From<LwwMap<String, TypedCrdt<V, N, S>, HybridLogicalTimestamp<N>, N>>
    for TypedCrdt<V, N, S>
where
    V: Eq + Hash + Clone,
    N: Ord + Clone,
    S: BuildHasher + Default,
{
    fn from(value: LwwMap<String, TypedCrdt<V, N, S>, HybridLogicalTimestamp<N>, N>) -> Self {
        Self::Map(value)
    }
}

impl<V, N, S> From<Register<V, VectorClock<N>>> for TypedCrdt<V, N, S>
where
    V: Eq + Hash + Clone,
    N: Ord + Clone,
    S: BuildHasher + Default,
{
    fn from(value: Register<V, VectorClock<N>>) -> Self {
        Self::Register(value)
    }
}

impl<V, N, S> From<Set<V, S>> for TypedCrdt<V, N, S>
where
    V: Eq + Hash + Clone,
    N: Ord + Clone,
    S: BuildHasher + Default,
{
    fn from(value: Set<V, S>) -> Self {
        Self::Set(value)
    }
}

impl<V, N, S> From<VectorData<N, V>> for TypedCrdt<V, N, S>
where
    V: Eq + Hash + Clone,
    N: Ord + Clone,
    S: BuildHasher + Default,
{
    fn from(value: VectorData<N, V>) -> Self {
        Self::VectorData(value)
    }
}

impl<V, N, S> CrdtUnion<N> for TypedCrdt<V, N, S>
where
    V: Eq + Hash + Clone + 'static,
    N: Ord + Clone + 'static,
    S: BuildHasher + Default + Clone + 'static,
{
    type DeltaRef<'d> = TypedCrdtRef<'d, V, N, S>;
    type DeltaMut<'d> = TypedCrdtMut<'d, V, N, S>;

    fn try_merge(a: &Self, b: &Self) -> Result<Self, IncompatibleType> {
        match (a, b) {
            (TypedCrdt::Map(map_a), TypedCrdt::Map(map_b)) => {
                Ok(TypedCrdt::Map(CrdtState::merge(map_a, map_b)))
            }
            (TypedCrdt::Register(reg_a), TypedCrdt::Register(reg_b)) => {
                Ok(TypedCrdt::Register(CrdtState::merge(reg_a, reg_b)))
            }
            (TypedCrdt::Set(set_a), TypedCrdt::Set(set_b)) => {
                Ok(TypedCrdt::Set(CrdtState::merge(set_a, set_b)))
            }
            (TypedCrdt::VectorData(vec_a), TypedCrdt::VectorData(vec_b)) => {
                Ok(TypedCrdt::VectorData(CrdtState::merge(vec_a, vec_b)))
            }
            _ => Err(IncompatibleType),
        }
    }

    fn create_ref<'d>(
        base: Option<&'d Self>,
        delta: Option<&'d Self>,
    ) -> Option<Self::DeltaRef<'d>> {
        match (base, delta) {
            (None, None) => None,
            (Some(Self::Map(_)) | None, Some(Self::Map(_)) | None) => {
                let base = match base {
                    Some(Self::Map(base)) => Some(base),
                    _ => None,
                };
                let delta = match delta {
                    Some(Self::Map(delta)) => Some(delta),
                    _ => None,
                };
                Some(TypedCrdtRef::Map(DeltaRef { base, delta }))
            }
            (Some(Self::Set(_)) | None, Some(Self::Set(_)) | None) => {
                let base = match base {
                    Some(Self::Set(base)) => Some(base),
                    _ => None,
                };
                let delta = match delta {
                    Some(Self::Set(delta)) => Some(delta),
                    _ => None,
                };
                Some(TypedCrdtRef::Set(DeltaRef { base, delta }))
            }
            (Some(Self::Register(_)) | None, Some(Self::Register(_)) | None) => {
                let base = match base {
                    Some(Self::Register(base)) => Some(base),
                    _ => None,
                };
                let delta = match delta {
                    Some(Self::Register(delta)) => Some(delta),
                    _ => None,
                };
                Some(TypedCrdtRef::Register(DeltaRef { base, delta }))
            }
            (Some(Self::VectorData(_)) | None, Some(Self::VectorData(_)) | None) => {
                let base = match base {
                    Some(Self::VectorData(base)) => Some(base),
                    _ => None,
                };
                let delta = match delta {
                    Some(Self::VectorData(delta)) => Some(delta),
                    _ => None,
                };
                Some(TypedCrdtRef::VectorData(DeltaRef { base, delta }))
            }
            _ => {
                debug_assert!(false, "Incompatible type!");
                None
            }
        }
    }

    fn create_mut<'d>(base: Option<&'d Self>, delta: &'d mut Self) -> Option<Self::DeltaMut<'d>> {
        match (base, delta) {
            (Some(Self::Map(_)) | None, Self::Map(delta)) => {
                let base = match base {
                    Some(Self::Map(value)) => Some(value),
                    _ => None,
                };
                Some(TypedCrdtMut::Map(DeltaMut { base, delta }))
            }
            (Some(Self::Register(_)) | None, Self::Register(delta)) => {
                let base = match base {
                    Some(Self::Register(value)) => Some(value),
                    _ => None,
                };
                Some(TypedCrdtMut::Register(DeltaMut { base, delta }))
            }
            (Some(Self::Set(_)) | None, Self::Set(delta)) => {
                let base = match base {
                    Some(Self::Set(value)) => Some(value),
                    _ => None,
                };
                Some(TypedCrdtMut::Set(DeltaMut { base, delta }))
            }
            (Some(Self::VectorData(_)) | None, Self::VectorData(delta)) => {
                let base = match base {
                    Some(Self::VectorData(value)) => Some(value),
                    _ => None,
                };
                Some(TypedCrdtMut::VectorData(DeltaMut { base, delta }))
            }
            _ => {
                debug_assert!(false, "Incompatible type!");
                None
            }
        }
    }

    fn create_matching_default(example: &Self) -> Self {
        match example {
            TypedCrdt::Map(_) => TypedCrdt::Map(Default::default()),
            TypedCrdt::Register(_) => TypedCrdt::Register(Default::default()),
            TypedCrdt::Set(_) => TypedCrdt::Set(Default::default()),
            TypedCrdt::VectorData(_) => TypedCrdt::VectorData(Default::default()),
        }
    }

    fn calculate_delta(&self, base_version: &VectorClock<N>) -> Self {
        match self {
            TypedCrdt::Map(map) => TypedCrdt::Map(map.calculate_delta(base_version)),
            TypedCrdt::Register(_) => self.clone(),
            TypedCrdt::Set(_) => self.clone(),
            TypedCrdt::VectorData(_) => self.clone(),
        }
    }
}

impl<V, N, S> ContentEq for TypedCrdt<V, N, S>
where
    V: Eq + Hash + Clone,
    N: Ord + Clone,
    S: BuildHasher + Default + Clone,
{
    fn content_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Map(a), Self::Map(b)) => a.content_eq(b),
            (Self::Register(a), Self::Register(b)) => a.content_eq(b),
            (Self::Set(a), Self::Set(b)) => a.content_eq(b),
            (Self::VectorData(a), Self::VectorData(b)) => a.content_eq(b),
            _ => false,
        }
    }
}

impl<V, N, S> HasPlainRepresentation<N> for TypedCrdt<V, N, S>
where
    V: Eq + Hash + Clone,
    N: Ord + Clone,
    S: BuildHasher + Default + Clone,
{
    type Plain = PlainTypes<N, V, S>;
    type Error = TypedError;

    fn init_from_plain(
        ctx: &impl UpdateContext<N>,
        plain: Self::Plain,
    ) -> Result<Self, Self::Error> {
        Ok(match plain {
            PlainTypes::Map(v) => {
                let mut map = LwwMap::default();
                let _ = map.apply_changes(ctx, v)?;
                Self::Map(map)
            }
            PlainTypes::Register(v) => {
                let mut crdt = Register::default();
                let _ = crdt.apply_changes(ctx, v)?;
                Self::Register(crdt)
            }
            PlainTypes::Set(v) => {
                let mut crdt = Set::default();
                let _ = crdt.apply_changes(v)?;
                Self::Set(crdt)
            }
            PlainTypes::VectorData(v) => {
                let mut crdt = VectorData::default();
                let _ = crdt.apply_changes(ctx, v)?;
                Self::VectorData(crdt)
            }
        })
    }
}

/// Error returned by [`TypedCrdtMut::apply_changes`]. In addition to the general `IncompatibleType`
/// error, this encodes [`HasPlainRepresentation::Error`] for each child enum variant.
#[derive(Debug, PartialEq, Eq)]
pub enum TypedError {
    /// Error while merging a child map. This is boxed to prevent infinite-sized type.
    Map(Box<LwwCrdtContainerApplyError<TypedError>>),
    /// Error while merging a child register.
    Register(RegisterError),
    /// Error while merging a child set.
    Set(GenerationExhausted),
    /// Error while merging a child vector data.
    VectorData(VectorDataError),
    /// Data cannot be merged because the children values are not of the same type.
    IncompatibleType,
}

impl From<LwwCrdtContainerApplyError<TypedError>> for TypedError {
    fn from(value: LwwCrdtContainerApplyError<TypedError>) -> Self {
        Self::Map(Box::new(value))
    }
}

impl From<RegisterError> for TypedError {
    fn from(value: RegisterError) -> Self {
        Self::Register(value)
    }
}

impl From<GenerationExhausted> for TypedError {
    fn from(value: GenerationExhausted) -> Self {
        Self::Set(value)
    }
}

impl From<VectorDataError> for TypedError {
    fn from(value: VectorDataError) -> Self {
        Self::VectorData(value)
    }
}

/// The plain types corresponding to a [`TypedCrdt`]. This encodes [`HasPlainRepresentation::Plain`]
/// for each child enum variant.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive_where(PartialEq, Eq; N, V)]

pub enum PlainTypes<N: Ord, V: Eq + Hash, S: BuildHasher + Default> {
    /// The plain type for `Map`. This should be the same as the return type of
    /// [`LwwMapRead::to_plain`].
    Map(BTreeMap<String, PlainTypes<N, V, S>>),
    /// The plain type for `Register`. This should be the same as the return type of
    /// [`RegisterRead::to_plain`].
    Register(Vec<V>),
    /// The plain type for `Set`. This should be the same as the return type of
    /// [`SetRead::to_plain`].
    Set(HashSet<V, S>),
    /// The plain type for `VectorData`. This should be the same as the return type of
    /// [`VectorDataRead::to_plain`].
    VectorData(BTreeMap<N, V>),
}

// Manually implement `Arbitrary` so it doesn't bound on `S: Arbitrary`.
impl<'a, N, V, S> Arbitrary<'a> for PlainTypes<N, V, S>
where
    N: Ord + Arbitrary<'a>,
    V: Eq + Hash + Arbitrary<'a>,
    S: BuildHasher + Default,
{
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(match u.int_in_range(0..=3)? {
            0 => Self::Map(u.arbitrary()?),
            1 => Self::Register(u.arbitrary()?),
            2 => Self::Set(u.arbitrary()?),
            3 => Self::VectorData(u.arbitrary()?),
            _ => unreachable!(),
        })
    }
}

/// Checker to validate the convergence of [`TypedCrdt`].
///
/// This is used in fuzz tests to make sure that [`TypedCrdt`] converges if [`TypedCrdt::try_merge`]
/// returns success.
///
/// Requires the feature _`checker`_.
#[cfg(any(test, feature = "checker"))]
pub mod checker {
    use arbitrary::Arbitrary;

    use crate::{
        checker::{test_fakes::TriState, utils::DeterministicHasher},
        lww_crdt_container::CrdtUnion,
        lww_map::LwwMap,
        register::Register,
        set::Set,
        vector_data::VectorData,
        CrdtState,
    };

    use super::TypedCrdt;

    /// A wrapper around [`TypedCrdt`] that makes it implement [`CrdtState`] (instead of just
    /// [`CrdtUnion`]).
    ///
    /// If merging results in an error, the wrapped value becomes `None`, and merging `None` with
    /// anything results in `None`.
    #[derive(Debug, Clone, PartialEq, Eq, Arbitrary)]
    pub struct CrdtUnionMergeResult(Option<TypedCrdt<TriState, u8, DeterministicHasher>>);

    impl CrdtState for CrdtUnionMergeResult {
        fn merge(a: &Self, b: &Self) -> Self {
            match (&a.0, &b.0) {
                (Some(v_a), Some(v_b)) => match CrdtUnion::try_merge(v_a, v_b) {
                    Ok(v) => CrdtUnionMergeResult(Some(v)),
                    Err(_) => CrdtUnionMergeResult(None),
                },
                _ => CrdtUnionMergeResult(None),
            }
        }

        fn is_valid_collection<'a>(collection: impl IntoIterator<Item = &'a Self>) -> bool
        where
            Self: 'a,
        {
            // Delegate to child types to check for convergence requirements
            let mut maps = Vec::new();
            let mut vector_datas = Vec::new();
            let mut registers = Vec::new();
            let mut sets = Vec::new();
            for item in collection {
                match &item.0 {
                    Some(TypedCrdt::Map(map)) => maps.push(map),
                    Some(TypedCrdt::VectorData(v)) => vector_datas.push(v),
                    Some(TypedCrdt::Register(reg)) => registers.push(reg),
                    Some(TypedCrdt::Set(set)) => sets.push(set),
                    None => {}
                }
            }
            LwwMap::is_valid_collection(maps)
                && VectorData::is_valid_collection(vector_datas)
                && Register::is_valid_collection(registers)
                && Set::is_valid_collection(sets)
        }
    }
}

#[cfg(feature = "proto")]
pub(crate) mod proto {
    use submerge_internal_proto::{
        protos::submerge as submerge_proto, FromProto, FromProtoError, NodeMapping, ToProto,
    };
    use thiserror::Error;

    use crate::{lww_map::LwwMap, register::Register, set::Set, vector_data::VectorData};

    use super::TypedCrdt;

    #[derive(Debug, Error)]
    pub(crate) enum TypedCrdtFromProtoError {
        #[error(transparent)]
        FromProtoError(#[from] FromProtoError),
        #[error("Unrecognized type")]
        UnrecognizedType,
    }

    impl TypedCrdt<Vec<u8>, String> {
        /// Construct a typed CRDT from the given `proto`.
        pub(crate) fn from_proto(
            proto: &submerge_proto::submerge_container::Value,
            node_ids: &[String],
        ) -> Result<Self, TypedCrdtFromProtoError> {
            Ok(match proto {
                submerge_proto::submerge_container::Value::Map(submerge_map) => {
                    TypedCrdt::Map(LwwMap::from_proto(submerge_map, node_ids)?)
                }
                submerge_proto::submerge_container::Value::Set(submerge_set) => {
                    TypedCrdt::Set(Set::from_proto(submerge_set, node_ids)?)
                }
                submerge_proto::submerge_container::Value::Register(submerge_register) => {
                    TypedCrdt::Register(Register::from_proto(submerge_register, node_ids)?)
                }
                submerge_proto::submerge_container::Value::VectorData(submerge_vector_data) => {
                    TypedCrdt::VectorData(VectorData::from_proto(submerge_vector_data, node_ids)?)
                }
                _ => Err(TypedCrdtFromProtoError::UnrecognizedType)?,
            })
        }
    }

    impl ToProto for TypedCrdt<Vec<u8>, String> {
        type Proto = submerge_proto::submerge_container::Value;

        fn to_proto(&self, node_ids: &mut NodeMapping<String>) -> Self::Proto {
            match self {
                TypedCrdt::Map(lww_map) => {
                    submerge_proto::submerge_container::Value::Map(lww_map.to_proto(node_ids))
                }
                TypedCrdt::Register(register) => {
                    submerge_proto::submerge_container::Value::Register(register.to_proto(node_ids))
                }
                TypedCrdt::Set(set) => {
                    submerge_proto::submerge_container::Value::Set(set.to_proto(node_ids))
                }
                TypedCrdt::VectorData(vector_data) => {
                    submerge_proto::submerge_container::Value::VectorData(
                        vector_data.to_proto(node_ids),
                    )
                }
            }
        }
    }

    #[cfg(test)]
    #[derive_fuzztest::proptest]
    fn typed_crdt_roundtrip(typed_crdt: TypedCrdt<Vec<u8>, String>) {
        let mut node_ids = NodeMapping::default();
        assert_eq!(
            TypedCrdt::from_proto(&typed_crdt.to_proto(&mut node_ids), &node_ids.into_vec())
                .unwrap(),
            typed_crdt
        );
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::register::RegisterRead;

    use super::TypedCrdt;

    #[test]
    fn json_decode() {
        let json = json!({
            "Register": [
                {
                    "value": "What is your #0 favorite emoji?",
                    "timestamp": {
                        "distributed_clock": {},
                        "hybrid_logical_timestamp": {
                            "logical_time": 1,
                            "causality": 0
                        }
                    }
                }
            ],
        });
        let typed_crdt = serde_json::from_value::<TypedCrdt<String, u8>>(json).unwrap();
        let TypedCrdt::Register(register) = typed_crdt else {
            unreachable!()
        };
        assert_eq!(register.get_all(), vec!["What is your #0 favorite emoji?"]);
    }
}
