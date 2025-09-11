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

use crdt::typed_crdt::{TypedCrdt, TypedCrdtMut};
use handle_map::HandleLike;
use jni::{
    objects::{JByteArray, JClass, JObject, JString, JThrowable},
    sys::jlong,
    JNIEnv,
};
use pourover::{exception::runtime_exception, jni_method};
use submerge::{
    CommitUpdateError, DataStore, DataStoreConfig, DeltaDocument, DocumentTransaction, SubmergeMap,
    SubmergeRegister, SubmergeSet, SubmergeVectorData,
};

use crate::{
    class::{
        exceptions::{
            merge_exception, older_base_needed_exception, transaction_exception,
            transaction_exception_with_cause,
        },
        storage_interface::StorageError,
    },
    utils::declare_handle,
};

use super::{
    document_transaction::DocumentTransactionHandle,
    exceptions::try_throwable_submerge,
    network_interface::{JavaNetwork, NetworkInterfaceJni},
    storage_interface::{JavaStorage, StorageInterfaceJni},
    timestamp_provider::{JavaTimestampProvider, TimestampProviderJni},
    version_vector::VersionVectorHandle,
};

/// A [`DataStoreConfig`] for use by Java.
pub enum JavaDataStoreConfig {}

impl DataStoreConfig for JavaDataStoreConfig {
    type NodeId = String;
    type ValueType = Vec<u8>;
    type Network = JavaNetwork;
    type Storage = JavaStorage;
    type TimestampProvider = JavaTimestampProvider;
}

pub type SubmergeJavaMap = SubmergeMap<JavaDataStoreConfig>;
pub type SubmergeJavaRegister = SubmergeRegister<JavaDataStoreConfig>;
pub type SubmergeJavaSet = SubmergeSet<JavaDataStoreConfig>;
pub type SubmergeJavaVectorData = SubmergeVectorData<JavaDataStoreConfig>;
pub type SubmergeJavaTypedCrdt = TypedCrdt<Vec<u8>, String>;
pub type SubmergeJavaTypedCrdtMut<'a> = TypedCrdtMut<'a, Vec<u8>, String>;

declare_handle! {
    pub struct DataStoreHandle for DataStore<JavaDataStoreConfig>;
}

#[jni_method(package = "com.google.android.submerge", class = "DataStore")]
extern "system" fn nativeInit<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    local_id: JString<'local>,
    network: NetworkInterfaceJni<JObject<'local>>,
    storage: StorageInterfaceJni<JObject<'local>>,
    timestamp_provider: TimestampProviderJni<JObject<'local>>,
) -> jlong {
    try_throwable_submerge(&mut env, |env| {
        let local_id = String::from(env.get_string(&local_id)?);
        let network = JavaNetwork::new(env, network)?;
        let storage = JavaStorage::new(env, storage)?;
        let timestamp_provider = JavaTimestampProvider::new(env, timestamp_provider)?;
        Ok(DataStoreHandle::allocate(|| {
            DataStore::new(local_id, network, storage, timestamp_provider)
        })?
        .jlong())
    })
}

#[jni_method(package = "com.google.android.submerge", class = "DataStore")]
extern "system" fn nativeClose<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    handle_id: jlong,
) {
    try_throwable_submerge(&mut env, |_env| {
        let handle = DataStoreHandle::try_from(handle_id)?;
        let _ = handle.deallocate()?;
        Ok(())
    })
}

#[jni_method(package = "com.google.android.submerge", class = "DataStore")]
extern "system" fn nativeNewDocumentTransaction<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    handle_id: jlong,
    doc_id: JString<'local>,
) -> jlong {
    try_throwable_submerge(&mut env, |env| {
        let handle = DataStoreHandle::try_from(handle_id)?;
        let doc_id: String = env.get_string(&doc_id)?.into();
        let mut data_store = handle.get_mut()?;
        let doc_handle = match data_store.open_or_create_document(doc_id) {
            Ok(doc_handle) => doc_handle,
            Err(StorageError::StorageException(e)) => Err(JThrowable::from(env.new_local_ref(e)?))?,
            Err(e) => Err(runtime_exception(e.to_string()))?,
        };
        Ok(DocumentTransactionHandle::allocate(|| DocumentTransaction::new(doc_handle))?.jlong())
    })
}

#[jni_method(package = "com.google.android.submerge", class = "DataStore")]
extern "system" fn nativeGetFullUpdateMessage<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    handle_id: jlong,
    doc_id: JString<'local>,
) -> JByteArray<'local> {
    try_throwable_submerge(&mut env, |env| {
        let handle = DataStoreHandle::try_from(handle_id)?;
        let doc_id: String = env.get_string(&doc_id)?.into();
        let data_store = handle.get()?;
        let snapshot = data_store.snapshot(doc_id);
        if let Some(document) = snapshot {
            let serialized_doc = document
                .into_full_delta()
                .to_proto_bytes()
                .map_err(runtime_exception)?;
            Ok(env.byte_array_from_slice(&serialized_doc)?)
        } else {
            Ok(JByteArray::default())
        }
    })
}

#[jni_method(package = "com.google.android.submerge", class = "DataStore")]
extern "system" fn nativeCommitNetworkUpdate<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    handle_id: jlong,
    doc_id: JString<'local>,
    update_message: JByteArray<'local>,
) {
    try_throwable_submerge(&mut env, |env| {
        let handle = DataStoreHandle::try_from(handle_id)?;
        let doc_id: String = env.get_string(&doc_id)?.into();
        let mut data_store = handle.get_mut()?;
        let mut doc_handle = match data_store.open_or_create_document(doc_id) {
            Ok(doc_handle) => doc_handle,
            Err(StorageError::StorageException(throwable)) => {
                Err(transaction_exception_with_cause(
                    env,
                    "Transaction failed",
                    <&JThrowable>::from(throwable.as_obj()),
                )?)?
            }
            Err(err) => Err(transaction_exception(err.to_string()))?,
        };

        let update_message = env.convert_byte_array(update_message)?;
        let delta = DeltaDocument::<JavaDataStoreConfig>::from_proto_bytes(&update_message)
            .map_err(runtime_exception)?;
        match doc_handle.commit_network_update(&delta) {
            Ok(_) => {}
            Err(CommitUpdateError::StorageError(StorageError::StorageException(throwable))) => {
                Err(transaction_exception_with_cause(
                    env,
                    "Transaction failed",
                    <&JThrowable>::from(throwable.as_obj()),
                )?)?
            }
            Err(CommitUpdateError::OlderBaseNeeded {
                required_base_version,
            }) => {
                let version_handle = VersionVectorHandle::allocate(|| required_base_version)?;
                Err(older_base_needed_exception(env, version_handle)?)?
            }
            Err(CommitUpdateError::MergeError(err)) => Err(merge_exception(err.to_string()))?,
            Err(err) => Err(transaction_exception(err.to_string()))?,
        }
        Ok(())
    })
}

#[jni_method(package = "com.google.android.submerge", class = "DataStore")]
extern "system" fn nativeCalculateDelta<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    handle_id: jlong,
    doc_id: JString<'local>,
    base_version_handle_id: jlong,
) -> JByteArray<'local> {
    try_throwable_submerge(&mut env, |env| {
        let handle = DataStoreHandle::try_from(handle_id)?;
        let doc_id: String = env.get_string(&doc_id)?.into();
        let data_store = handle.get()?;
        let snapshot = data_store.snapshot(doc_id);
        if let Some(document) = snapshot {
            let version = VersionVectorHandle::try_from(base_version_handle_id)?
                .get()?
                .clone();
            let delta = document.calculate_delta(version.clone());
            let serialized_delta = delta.to_proto_bytes().map_err(runtime_exception)?;
            Ok(env.byte_array_from_slice(&serialized_delta)?)
        } else {
            Ok(JByteArray::default())
        }
    })
}

#[jni_method(package = "com.google.android.submerge", class = "DataStore")]
extern "system" fn nativeGetDocumentVersion<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    handle_id: jlong,
    doc_id: JString<'local>,
) -> jlong {
    try_throwable_submerge(&mut env, |env| {
        let handle = DataStoreHandle::try_from(handle_id)?;
        let doc_id: String = env.get_string(&doc_id)?.into();
        let data_store = handle.get()?;
        let snapshot = data_store.snapshot(doc_id);
        let version = snapshot
            .map(|document| document.version)
            .unwrap_or_default();
        Ok(VersionVectorHandle::allocate(|| version)?.jlong())
    })
}
