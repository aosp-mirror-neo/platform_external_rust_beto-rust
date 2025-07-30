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

use std::{
    hash::{BuildHasher, Hash},
    ops::{Deref, DerefMut},
    sync::Arc,
};

use crdt::{
    lww_crdt_container::CrdtUnion,
    typed_crdt::{TypedCrdt, TypedCrdtMut, TypedCrdtRef},
};
use distributed_time::TimestampOverflow;
use handle_map::{HandleLike, HandleNotPresentError};
use jni::{
    objects::{JClass, JString},
    sys::{jint, jlong},
    JNIEnv,
};
use parking_lot::{ArcRwLockWriteGuard, RwLock};
use pourover::{exception::runtime_exception, jni_method};
use submerge::UpdateContext;

use crate::{
    class::{
        data_store::{JavaDataStoreConfig, SubmergeJavaTypedCrdt, SubmergeJavaTypedCrdtMut},
        document_transaction::{DocumentTransactionHandle, JavaDocumentTransaction},
        exceptions::{timestamp_overflow_exception, try_throwable_submerge, SubmergeJavaError},
    },
    utils::declare_handle,
};

#[derive(Debug, Clone)]
pub enum CrdtType {
    Map,
    Register,
    Set,
    VectorData,
}

impl<V, N, S> From<&TypedCrdt<V, N, S>> for CrdtType
where
    V: Eq + Hash + Clone,
    N: Ord + Clone,
    S: BuildHasher + Default,
{
    fn from(value: &TypedCrdt<V, N, S>) -> Self {
        match value {
            TypedCrdt::Map(_) => CrdtType::Map,
            TypedCrdt::Register(_) => CrdtType::Register,
            TypedCrdt::Set(_) => CrdtType::Set,
            TypedCrdt::VectorData(_) => CrdtType::VectorData,
        }
    }
}

impl<'d, V, N, S> From<TypedCrdtRef<'d, V, N, S>> for CrdtType
where
    V: Eq + Hash + Clone,
    N: Ord + Clone,
    S: BuildHasher + Default,
{
    fn from(value: TypedCrdtRef<V, N, S>) -> Self {
        match value {
            TypedCrdtRef::Map(_) => CrdtType::Map,
            TypedCrdtRef::Register(_) => CrdtType::Register,
            TypedCrdtRef::Set(_) => CrdtType::Set,
            TypedCrdtRef::VectorData(_) => CrdtType::VectorData,
        }
    }
}

impl<'d, V, N, S> From<TypedCrdtMut<'d, V, N, S>> for CrdtType
where
    V: Eq + Hash + Clone,
    N: Ord + Clone,
    S: BuildHasher + Default,
{
    fn from(value: TypedCrdtMut<V, N, S>) -> Self {
        match value {
            TypedCrdtMut::Map(_) => CrdtType::Map,
            TypedCrdtMut::Register(_) => CrdtType::Register,
            TypedCrdtMut::Set(_) => CrdtType::Set,
            TypedCrdtMut::VectorData(_) => CrdtType::VectorData,
        }
    }
}

/// The root of a CRDT tree.
#[derive(Debug, Clone)]
pub enum TreeRoot {
    Document(DocumentTransactionHandle),
    Detached {
        root: Arc<RwLock<Option<SubmergeJavaTypedCrdt>>>,
        doc_handle: DocumentTransactionHandle,
    },
}

/// A reference to a node in a CRDT tree
///
/// This contains enough information to look up the corresponding [`TypedCrdtMut`] in the tree, and
/// is the type exposed to the Java class `SubmergeDataType` (via a handle abstraction).
#[derive(Debug, Clone)]
pub struct JavaSubmergeDataType {
    /// The path used to look up the node from the `root`.
    ///
    /// An empty vec corresponds to the node itself, and each element in the vec corresponds to one
    /// layer down the tree.
    pub path: Vec<String>,
    /// The type of the referenced CRDT node.
    ///
    /// This is used to identify the referenced CRDT type to Java so it can create the corresponding
    /// Java wrapper.
    pub crdt_type: Option<CrdtType>,
    /// The root to look up the node from.
    ///
    /// This can either be a document ([`DocumentTransactionHandle`]) or a detached [`TypedCrdt`].
    pub root_handle: TreeRoot,
}

impl JavaSubmergeDataType {
    pub fn new_detached(
        typed_crdt: SubmergeJavaTypedCrdt,
        doc_handle: DocumentTransactionHandle,
    ) -> Self {
        debug_assert!(doc_handle.get().is_ok());
        JavaSubmergeDataType {
            path: vec![],
            crdt_type: Some((&typed_crdt).into()),
            root_handle: TreeRoot::Detached {
                root: Arc::new(RwLock::new(Some(typed_crdt))),
                doc_handle,
            },
        }
    }
}

declare_handle! {
    pub struct SubmergeDataTypeHandle for JavaSubmergeDataType;
}

/// A guard object returned while reading a [`JavaSubmergeDataType`].
///
/// # Params
/// * `R` - the type of read guard returned from [`handle_map::HandleMap::get`].
/// * `W` - the type of write guard returned from [`handle_map::HandleMap::get_mut`].
pub enum SubmergeDataTypeWriteGuard<'a, R, W>
where
    R: Deref<Target = JavaDocumentTransaction>,
    W: DerefMut<Target = JavaDocumentTransaction>,
{
    Document {
        submerge_data_type: &'a JavaSubmergeDataType,
        doc: W,
    },
    Detached {
        submerge_data_type: &'a JavaSubmergeDataType,
        doc: R,
        root: ArcRwLockWriteGuard<parking_lot::RawRwLock, Option<SubmergeJavaTypedCrdt>>,
    },
}

impl<'a, R, W> SubmergeDataTypeWriteGuard<'a, R, W>
where
    R: Deref<Target = JavaDocumentTransaction>,
    W: DerefMut<Target = JavaDocumentTransaction>,
{
    pub fn get_context_and_node_mut(
        &mut self,
    ) -> Result<
        (
            Option<SubmergeJavaTypedCrdtMut>,
            &UpdateContext<JavaDataStoreConfig>,
        ),
        AccessError,
    > {
        match self {
            SubmergeDataTypeWriteGuard::Document {
                submerge_data_type,
                doc,
            } => {
                let (ctx, root) = doc.get_context_and_root_mut();
                let node = submerge_data_type.access_mut(root.into_value_mut(ctx)?, ctx)?;
                Ok((node, ctx))
            }
            SubmergeDataTypeWriteGuard::Detached {
                submerge_data_type,
                doc,
                root: owned,
            } => {
                let ctx = doc.update_context();
                let node = submerge_data_type.access_mut(
                    owned.as_mut().and_then(|v| TypedCrdt::create_mut(None, v)),
                    ctx,
                )?;
                Ok((node, ctx))
            }
        }
    }
}

#[derive(Debug)]
pub enum AccessError {
    TimestampOverflow,
    TypeMismatch,
}

impl From<TimestampOverflow> for AccessError {
    fn from(_: TimestampOverflow) -> Self {
        Self::TimestampOverflow
    }
}

impl From<AccessError> for SubmergeJavaError<'_> {
    fn from(value: AccessError) -> Self {
        match value {
            AccessError::TimestampOverflow => {
                timestamp_overflow_exception("Timestamp overflow").into()
            }
            AccessError::TypeMismatch => runtime_exception("Type mismatch").into(),
        }
    }
}

impl JavaSubmergeDataType {
    pub fn lock_write(
        &self,
    ) -> Result<
        SubmergeDataTypeWriteGuard<
            impl Deref<Target = JavaDocumentTransaction> + '_,
            impl DerefMut<Target = JavaDocumentTransaction> + '_,
        >,
        HandleNotPresentError,
    > {
        match &self.root_handle {
            TreeRoot::Document(doc_handle) => {
                let doc = doc_handle.get_mut()?;
                Ok(SubmergeDataTypeWriteGuard::Document {
                    submerge_data_type: self,
                    doc,
                })
            }
            TreeRoot::Detached { root, doc_handle } => {
                let root_guard = root.write_arc();
                let doc = doc_handle.get()?;
                Ok(SubmergeDataTypeWriteGuard::Detached {
                    submerge_data_type: self,
                    doc,
                    root: root_guard,
                })
            }
        }
    }

    fn access_mut<'a>(
        &self,
        root: Option<SubmergeJavaTypedCrdtMut<'a>>,
        ctx: &UpdateContext<JavaDataStoreConfig>,
    ) -> Result<Option<SubmergeJavaTypedCrdtMut<'a>>, AccessError> {
        let mut node = root;
        for key in &self.path {
            let Some(TypedCrdtMut::Map(map)) = node else {
                return Err(AccessError::TypeMismatch);
            };
            node = map.into_container_mut(key.clone()).into_value_mut(ctx)?;
        }
        Ok(node)
    }
}

#[jni_method(package = "com.google.android.submerge", class = "SubmergeDataType")]
extern "system" fn nativeGetCrdtType<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    data_handle_id: jlong,
) -> jint {
    try_throwable_submerge(&mut env, |_env| {
        let data_handle = SubmergeDataTypeHandle::try_from(data_handle_id)?;
        let data_type = data_handle.get()?;
        Ok(match data_type.crdt_type {
            Some(CrdtType::Map) => 1,
            Some(CrdtType::Set) => 2,
            Some(CrdtType::Register) => 3,
            Some(CrdtType::VectorData) => 4,
            None => 0,
        })
    })
}

#[jni_method(package = "com.google.android.submerge", class = "SubmergeDataType")]
extern "system" fn nativeToString<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    data_handle_id: jlong,
) -> JString<'local> {
    try_throwable_submerge(&mut env, |env| {
        let data_handle = SubmergeDataTypeHandle::try_from(data_handle_id)?;
        let data = data_handle.get()?;
        let mut guard = data.lock_write()?;
        let (typed_crdt, _) = guard.get_context_and_node_mut()?;
        Ok(env.new_string(format!("{:?}: {:?}", &*data, typed_crdt))?)
    })
}

#[jni_method(package = "com.google.android.submerge", class = "SubmergeDataType")]
extern "system" fn nativeClose<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    data_handle_id: jlong,
) {
    try_throwable_submerge(&mut env, |_env| {
        let data_handle = SubmergeDataTypeHandle::try_from(data_handle_id)?;
        let _ = data_handle.deallocate()?;
        Ok(())
    })
}

pub(crate) fn type_string(typed_crdt: Option<SubmergeJavaTypedCrdtMut>) -> &'static str {
    match typed_crdt {
        Some(TypedCrdtMut::Map(_)) => "Map",
        Some(TypedCrdtMut::Set(_)) => "Set",
        Some(TypedCrdtMut::Register(_)) => "Register",
        Some(TypedCrdtMut::VectorData(_)) => "VectorData",
        None => "None",
    }
}
