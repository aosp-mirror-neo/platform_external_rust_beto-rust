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

use std::{ops::Deref, sync::Arc};

use crdt::lww_crdt_container::{LwwCrdtContainerRead, LwwCrdtContainerWrite};
use handle_map::HandleLike;
use jni::{
    objects::{JByteArray, JClass, JString, JThrowable},
    sys::jlong,
    JNIEnv,
};
use pourover::{exception::runtime_exception, jni_method};
use submerge::{
    CommitUpdateError, DeltaDocument, DocumentHandle, DocumentTransaction, TransactionError,
};

use crate::{
    class::{
        exceptions::{
            older_base_needed_exception, ownership_exception, transaction_exception,
            transaction_exception_with_cause,
        },
        storage_interface::StorageError,
        submerge_data_type::{CrdtType, JavaSubmergeDataType, SubmergeDataTypeHandle, TreeRoot},
        version_vector::VersionVectorHandle,
    },
    utils::declare_handle,
};

use super::{
    data_store::{
        JavaDataStoreConfig, SubmergeJavaMap, SubmergeJavaRegister, SubmergeJavaSet,
        SubmergeJavaVectorData,
    },
    exceptions::try_throwable_submerge,
};

pub(crate) type JavaDocumentTransaction =
    DocumentTransaction<DocumentHandle<JavaDataStoreConfig>, JavaDataStoreConfig>;

declare_handle! {
    pub struct DocumentTransactionHandle for JavaDocumentTransaction;
}

#[jni_method(package = "com.google.android.submerge", class = "DocumentTransaction")]
extern "system" fn nativeGetRoot<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    handle_id: jlong,
) -> jlong {
    try_throwable_submerge(&mut env, |_env| {
        let handle = DocumentTransactionHandle::try_from(handle_id)?;
        let mut transaction = handle.get_mut()?;
        let (_ctx, root) = transaction.get_context_and_root_mut();
        let crdt_type = root.get_value().map(CrdtType::from);
        Ok(SubmergeDataTypeHandle::allocate(|| JavaSubmergeDataType {
            path: vec![],
            crdt_type,
            root_handle: TreeRoot::Document(handle),
        })?
        .jlong())
    })
}

#[jni_method(package = "com.google.android.submerge", class = "DocumentTransaction")]
extern "system" fn nativeSetRoot<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    handle_id: jlong,
    root_handle_id: jlong,
) -> jlong {
    try_throwable_submerge(&mut env, |_env| {
        let handle = DocumentTransactionHandle::try_from(handle_id)?;
        let mut transaction = handle.get_mut()?;
        let (ctx, mut root) = transaction.get_context_and_root_mut();
        let typed_crdt = if let Ok(root_handle) = SubmergeDataTypeHandle::try_from(root_handle_id) {
            let owned_data_type = root_handle.deallocate()?;
            let TreeRoot::Detached { root, .. } = owned_data_type.root_handle else {
                Err(ownership_exception(
                    "Cannot set root which is already attached",
                ))?
            };
            Arc::into_inner(root)
                .ok_or_else(|| ownership_exception("Outstanding refs, cannot take ownership"))?
                .into_inner()
        } else {
            None
        };
        let crdt_type = typed_crdt.as_ref().map(CrdtType::from);
        match typed_crdt {
            Some(v) => root.set_value(ctx, v)?,
            None => root.remove_value(ctx)?,
        }
        Ok(SubmergeDataTypeHandle::allocate(|| JavaSubmergeDataType {
            path: vec![],
            crdt_type,
            root_handle: TreeRoot::Document(handle),
        })?
        .jlong())
    })
}

#[jni_method(package = "com.google.android.submerge", class = "DocumentTransaction")]
extern "system" fn nativeClose<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    handle_id: jlong,
) {
    try_throwable_submerge(&mut env, |_env| {
        let handle = DocumentTransactionHandle::try_from(handle_id)?;
        let _ = handle.deallocate()?;
        Ok(())
    })
}

#[jni_method(package = "com.google.android.submerge", class = "DocumentTransaction")]
extern "system" fn nativeCommit<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    handle_id: jlong,
) {
    try_throwable_submerge(&mut env, |env| {
        let handle = DocumentTransactionHandle::try_from(handle_id)
            .map_err(|_| runtime_exception("Cannot commit the same transaction twice"))?;
        let transaction = handle.deallocate()?;
        match transaction.commit() {
            Ok(_) => {}
            Err(TransactionError::StorageError(StorageError::StorageException(throwable))) => {
                Err(transaction_exception_with_cause(
                    env,
                    "Transaction failed",
                    <&JThrowable>::from(throwable.as_obj()),
                )?)?
            }
            Err(err) => Err(transaction_exception(err.to_string()))?,
        }
        Ok(())
    })
}

#[jni_method(package = "com.google.android.submerge", class = "DocumentTransaction")]
extern "system" fn nativeMergeNetworkUpdate<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    handle_id: jlong,
    update_message: JByteArray<'local>,
) {
    try_throwable_submerge(&mut env, |env| {
        let handle = DocumentTransactionHandle::try_from(handle_id)?;
        let mut transaction = handle.get_mut()?;
        let update_message = env.convert_byte_array(update_message)?;
        let delta = DeltaDocument::<JavaDataStoreConfig>::from_proto_bytes(&update_message)
            .map_err(runtime_exception)?;
        match transaction.merge_network_delta(&delta) {
            Ok(_) => Ok(()),
            Err(CommitUpdateError::OlderBaseNeeded {
                required_base_version,
            }) => {
                let version_handle = VersionVectorHandle::allocate(|| required_base_version)?;
                Err(older_base_needed_exception(env, version_handle)?)?
            }
            Err(err) => Err(runtime_exception(err.to_string()))?,
        }
    })
}

#[jni_method(package = "com.google.android.submerge", class = "DocumentTransaction")]
extern "system" fn nativeToString<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    handle_id: jlong,
) -> JString<'local> {
    try_throwable_submerge(&mut env, |env| {
        let handle = DocumentTransactionHandle::try_from(handle_id)?;
        let transaction_result = handle.get();
        Ok(match transaction_result {
            Ok(transaction) => env.new_string(format!("{:?}", transaction.deref()))?,
            Err(_) => env.new_string("Handle not present")?,
        })
    })
}

#[jni_method(package = "com.google.android.submerge", class = "DocumentTransaction")]
extern "system" fn nativeNewMap<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    handle_id: jlong,
) -> jlong {
    try_throwable_submerge(&mut env, |_env| {
        let handle = DocumentTransactionHandle::try_from(handle_id)?;
        Ok(SubmergeDataTypeHandle::allocate(|| {
            JavaSubmergeDataType::new_detached(SubmergeJavaMap::default().into(), handle)
        })?
        .jlong())
    })
}

#[jni_method(package = "com.google.android.submerge", class = "DocumentTransaction")]
extern "system" fn nativeNewRegister<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    handle_id: jlong,
) -> jlong {
    try_throwable_submerge(&mut env, |_env| {
        let handle = DocumentTransactionHandle::try_from(handle_id)?;
        Ok(SubmergeDataTypeHandle::allocate(|| {
            JavaSubmergeDataType::new_detached(SubmergeJavaRegister::default().into(), handle)
        })?
        .jlong())
    })
}

#[jni_method(package = "com.google.android.submerge", class = "DocumentTransaction")]
extern "system" fn nativeNewSet<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    handle_id: jlong,
) -> jlong {
    try_throwable_submerge(&mut env, |_env| {
        let handle = DocumentTransactionHandle::try_from(handle_id)?;
        Ok(SubmergeDataTypeHandle::allocate(|| {
            JavaSubmergeDataType::new_detached(SubmergeJavaSet::default().into(), handle)
        })?
        .jlong())
    })
}

#[jni_method(package = "com.google.android.submerge", class = "DocumentTransaction")]
extern "system" fn nativeNewVectorData<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    handle_id: jlong,
) -> jlong {
    try_throwable_submerge(&mut env, |_env| {
        let handle = DocumentTransactionHandle::try_from(handle_id)?;
        Ok(SubmergeDataTypeHandle::allocate(|| {
            JavaSubmergeDataType::new_detached(SubmergeJavaVectorData::default().into(), handle)
        })?
        .jlong())
    })
}
