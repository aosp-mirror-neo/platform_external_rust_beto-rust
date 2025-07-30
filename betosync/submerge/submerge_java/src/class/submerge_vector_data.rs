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

use crdt::{
    typed_crdt::TypedCrdtMut,
    vector_data::{VectorDataRead, VectorDataWrite},
};
use handle_map::HandleLike;
use jni::{
    objects::{JByteArray, JClass, JObject},
    sys::{jboolean, jint, jlong, JNI_FALSE, JNI_TRUE},
    JNIEnv,
};
use pourover::{exception::runtime_exception, jni_method};

use crate::{
    class::{exceptions::try_throwable_submerge, submerge_data_type::SubmergeDataTypeHandle},
    java_map::HashMapJni,
};

use super::submerge_data_type::type_string;

#[jni_method(package = "com.google.android.submerge", class = "SubmergeVectorData")]
extern "system" fn nativeSet<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    data_handle_id: jlong,
    value: JByteArray<'local>,
) -> jboolean {
    try_throwable_submerge(&mut env, |env| {
        let data_handle = SubmergeDataTypeHandle::try_from(data_handle_id)?;
        let data = data_handle.get()?;
        let mut guard = data.lock_write()?;
        let (typed_crdt, ctx) = guard.get_context_and_node_mut()?;
        let Some(TypedCrdtMut::VectorData(mut vector_data)) = typed_crdt else {
            Err(runtime_exception(format!(
                "Expected VectorData but found {}",
                type_string(typed_crdt)
            )))?
        };
        let value = env.convert_byte_array(value)?;
        let changed = vector_data.set(ctx.node_id.clone(), Some(value))?;
        Ok(if changed { JNI_TRUE } else { JNI_FALSE })
    })
}

#[jni_method(package = "com.google.android.submerge", class = "SubmergeVectorData")]
extern "system" fn nativeBumpVersionNumberForTesting<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    data_handle_id: jlong,
    increment: jint,
) {
    try_throwable_submerge(&mut env, |_env| {
        let data_handle = SubmergeDataTypeHandle::try_from(data_handle_id)?;
        let data = data_handle.get()?;
        let mut guard = data.lock_write()?;
        let (typed_crdt, ctx) = guard.get_context_and_node_mut()?;
        let Some(TypedCrdtMut::VectorData(mut vector_data)) = typed_crdt else {
            Err(runtime_exception(format!(
                "Expected VectorData but found {}",
                type_string(typed_crdt)
            )))?
        };
        #[allow(clippy::cast_sign_loss)]
        vector_data.bump_version_number_for_testing(ctx.node_id.clone(), increment as u32)?;
        Ok(())
    })
}

#[jni_method(package = "com.google.android.submerge", class = "SubmergeVectorData")]
extern "system" fn nativeEntries<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    data_handle_id: jlong,
) -> HashMapJni<JObject<'local>> {
    try_throwable_submerge(&mut env, |env| {
        let data_handle = SubmergeDataTypeHandle::try_from(data_handle_id)?;
        let data = data_handle.get()?;
        let mut guard = data.lock_write()?;
        let (typed_crdt, _) = guard.get_context_and_node_mut()?;

        let Some(TypedCrdtMut::VectorData(vector_data)) = typed_crdt else {
            Err(runtime_exception(format!(
                "Expected VectorData but found {}",
                type_string(typed_crdt)
            )))?
        };
        let entries = vector_data.entries();
        let java_map = HashMapJni::construct(
            env,
            entries.len().try_into().expect("Size should fit in i32"),
        )?;
        for (key, value) in entries {
            let key = env.new_string(key)?;
            let value = env.byte_array_from_slice(value)?;
            let _ = java_map.put(env, key, value)?;
        }
        Ok(java_map)
    })
}
