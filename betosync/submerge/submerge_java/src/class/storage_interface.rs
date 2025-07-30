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

use std::sync::Arc;

use jni::{
    objects::{GlobalRef, JObject},
    JNIEnv, JavaVM,
};
use pourover::{desc::ClassDesc, exception::JniResultExt};
use submerge::{proto::ProtoSerdeError, Document, StorageInterface};
use thiserror::Error;

use super::data_store::JavaDataStoreConfig;

/// A wrapper struct around a Java-implemented storage interface that implements
/// [`StorageInterface`].
#[derive(Debug, Clone)]
pub struct JavaStorage {
    jobject: StorageInterfaceJni<GlobalRef>,
    vm: Arc<JavaVM>,
}

impl JavaStorage {
    pub fn new<'local>(
        env: &mut JNIEnv<'local>,
        storage: StorageInterfaceJni<JObject<'local>>,
    ) -> Result<Self, jni::errors::Error> {
        Ok(Self {
            jobject: storage.to_global_ref(env)?,
            vm: Arc::new(env.get_java_vm()?),
        })
    }
}

impl StorageInterface<JavaDataStoreConfig> for JavaStorage {
    type Error = StorageError;

    fn on_new_update(
        &mut self,
        doc_id: &str,
        document: &Document<JavaDataStoreConfig>,
    ) -> Result<(), StorageError> {
        let mut env = self.vm.get_env()?;
        self.jobject
            .on_new_update(&mut env, doc_id, &document.to_proto_bytes()?)
            .extract_exception(
                &mut env,
                "com/google/android/submerge/StorageInterface$StorageException",
            )?
            .map_err(|throwable| match env.new_global_ref(throwable) {
                Ok(global_throwable) => StorageError::StorageException(global_throwable),
                Err(err) => StorageError::JniFailure(err),
            })?;
        Ok(())
    }

    fn read_from_storage(
        &self,
        doc_id: &str,
    ) -> Result<Option<Document<JavaDataStoreConfig>>, StorageError> {
        let mut env = self.vm.get_env()?;
        Ok(
            if let Some(bytes) = self.jobject.read_from_storage(&mut env, doc_id)? {
                Some(Document::from_proto_bytes(&bytes)?)
            } else {
                None
            },
        )
    }
}

/// A transparent wrapper over a Java implemented `StorageInterface`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct StorageInterfaceJni<Obj>(Obj);

static STORAGE_INTERFACE_CLASS: ClassDesc =
    ClassDesc::new("com/google/android/submerge/StorageInterface");

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Failed to serialize / deserialize document")]
    SerdeFailed(#[from] ProtoSerdeError),
    #[error("Failed to store to storage")]
    StorageException(GlobalRef),
    #[error(transparent)]
    JniFailure(#[from] jni::errors::Error),
}

impl From<protobuf::Error> for StorageError {
    fn from(value: protobuf::Error) -> Self {
        StorageError::SerdeFailed(ProtoSerdeError::ProtobufError(value))
    }
}

impl<'local, O: AsRef<JObject<'local>>> StorageInterfaceJni<O> {
    pub fn to_global_ref(
        &self,
        env: &mut JNIEnv<'_>,
    ) -> Result<StorageInterfaceJni<GlobalRef>, jni::errors::Error> {
        Ok(StorageInterfaceJni(env.new_global_ref(&self.0)?))
    }

    pub fn on_new_update(
        &self,
        env: &mut JNIEnv<'_>,
        doc_id: &str,
        serialized_doc: &[u8],
    ) -> Result<(), jni::errors::Error> {
        let serialized_doc = env.auto_local(env.byte_array_from_slice(serialized_doc)?);
        let doc_id = env.new_string(doc_id)?;
        pourover::call_method!(
            env,
            STORAGE_INTERFACE_CLASS,
            "onNewUpdate",
            "(Ljava/lang/String;[B)V",
            self.0.as_ref(),
            &doc_id,
            &serialized_doc
        )
    }

    pub fn read_from_storage(
        &self,
        env: &mut JNIEnv<'_>,
        doc_id: &str,
    ) -> Result<Option<Vec<u8>>, jni::errors::Error> {
        let doc_id = env.new_string(doc_id)?;
        match pourover::call_method!(
            env,
            STORAGE_INTERFACE_CLASS,
            "readFromStorage",
            "(Ljava/lang/String;)[B",
            self.0.as_ref(),
            &doc_id,
        ) {
            Ok(bytes) => {
                if bytes.is_null() {
                    Ok(None)
                } else {
                    let bytes = env.convert_byte_array(bytes)?;
                    Ok(Some(bytes))
                }
            }
            Err(e) => Err(e),
        }
    }
}
