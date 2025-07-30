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
    set::{SetRead, SetWrite},
    typed_crdt::TypedCrdtMut,
};
use handle_map::HandleLike;
use jni::{
    objects::{JByteArray, JClass, JObjectArray},
    sys::{jboolean, jint, jlong, JNI_FALSE, JNI_TRUE},
    JNIEnv,
};
use pourover::{exception::runtime_exception, jni_method};

use crate::{
    class::{exceptions::try_throwable_submerge, submerge_data_type::SubmergeDataTypeHandle},
    utils::collect_to_jarray,
};

use super::submerge_data_type::type_string;

#[jni_method(package = "com.google.android.submerge", class = "SubmergeSet")]
extern "system" fn nativeAdd<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    data_handle_id: jlong,
    value: JByteArray<'local>,
) -> jboolean {
    try_throwable_submerge(&mut env, |env| {
        let data_handle = SubmergeDataTypeHandle::try_from(data_handle_id)?;
        let data = data_handle.get()?;
        let mut guard = data.lock_write()?;
        let (typed_crdt, _) = guard.get_context_and_node_mut()?;
        let Some(TypedCrdtMut::Set(mut set)) = typed_crdt else {
            Err(runtime_exception(format!(
                "Expected Set but found {}",
                type_string(typed_crdt)
            )))?
        };
        let value = env.convert_byte_array(value)?;
        let added = set.add(value)?;
        Ok(if added { JNI_TRUE } else { JNI_FALSE })
    })
}

#[jni_method(package = "com.google.android.submerge", class = "SubmergeSet")]
extern "system" fn nativeRemove<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    data_handle_id: jlong,
    value: JByteArray<'local>,
) -> jboolean {
    try_throwable_submerge(&mut env, |env| {
        let data_handle = SubmergeDataTypeHandle::try_from(data_handle_id)?;
        let data = data_handle.get()?;
        let mut guard = data.lock_write()?;
        let (typed_crdt, _) = guard.get_context_and_node_mut()?;
        let Some(TypedCrdtMut::Set(mut set)) = typed_crdt else {
            Err(runtime_exception(format!(
                "Expected Set but found {}",
                type_string(typed_crdt)
            )))?
        };
        let value = env.convert_byte_array(value)?;
        let removed = set.remove(value);
        Ok(if removed { JNI_TRUE } else { JNI_FALSE })
    })
}

#[jni_method(package = "com.google.android.submerge", class = "SubmergeSet")]
extern "system" fn nativeBumpGenerationNumberForTesting<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    data_handle_id: jlong,
    value: JByteArray<'local>,
    increment: jint,
) {
    try_throwable_submerge(&mut env, |env| {
        let data_handle = SubmergeDataTypeHandle::try_from(data_handle_id)?;
        let data = data_handle.get()?;
        let mut guard = data.lock_write()?;
        let (typed_crdt, _) = guard.get_context_and_node_mut()?;
        let Some(TypedCrdtMut::Set(mut set)) = typed_crdt else {
            Err(runtime_exception(format!(
                "Expected Set but found {}",
                type_string(typed_crdt)
            )))?
        };
        let value = env.convert_byte_array(value)?;
        #[allow(clippy::cast_sign_loss)]
        set.bump_generation_number_for_testing(value, increment as u32)?;
        Ok(())
    })
}

#[jni_method(package = "com.google.android.submerge", class = "SubmergeSet")]
extern "system" fn nativeEntries<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    data_handle_id: jlong,
) -> JObjectArray<'local> {
    try_throwable_submerge(&mut env, |env| {
        let data_handle = SubmergeDataTypeHandle::try_from(data_handle_id)?;
        let data = data_handle.get()?;
        let mut guard = data.lock_write()?;
        let (typed_crdt, _) = guard.get_context_and_node_mut()?;
        let Some(TypedCrdtMut::Set(set)) = typed_crdt else {
            Err(runtime_exception(format!(
                "Expected Set but found {}",
                type_string(typed_crdt)
            )))?
        };
        Ok(collect_to_jarray(
            env,
            "[B",
            set.entries().into_iter(),
            |env, item| Ok(env.auto_local(env.byte_array_from_slice(item)?)),
        )?)
    })
}

#[jni_method(package = "com.google.android.submerge", class = "SubmergeSet")]
extern "system" fn nativeContains<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    data_handle_id: jlong,
    value: JByteArray<'local>,
) -> jboolean {
    try_throwable_submerge(&mut env, |env| {
        let data_handle = SubmergeDataTypeHandle::try_from(data_handle_id)?;
        let data = data_handle.get()?;
        let mut guard = data.lock_write()?;
        let (typed_crdt, _) = guard.get_context_and_node_mut()?;
        let Some(TypedCrdtMut::Set(set)) = typed_crdt else {
            Err(runtime_exception(format!(
                "Expected Set but found {}",
                type_string(typed_crdt)
            )))?
        };
        let value = env.convert_byte_array(value)?;
        Ok(set.contains(&value).into())
    })
}
