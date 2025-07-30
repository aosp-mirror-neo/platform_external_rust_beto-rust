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
    register::{RegisterRead, RegisterWrite},
    typed_crdt::TypedCrdtMut,
};
use handle_map::HandleLike;
use jni::{
    objects::{JByteArray, JClass, JObjectArray},
    sys::jlong,
    JNIEnv,
};
use pourover::{exception::runtime_exception, jni_method};

use crate::{
    class::{exceptions::try_throwable_submerge, submerge_data_type::SubmergeDataTypeHandle},
    utils::collect_to_jarray,
};

use super::submerge_data_type::type_string;

#[jni_method(package = "com.google.android.submerge", class = "SubmergeRegister")]
extern "system" fn nativeSet<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    data_handle_id: jlong,
    value: JByteArray<'local>,
) {
    try_throwable_submerge(&mut env, |env| {
        let data_handle = SubmergeDataTypeHandle::try_from(data_handle_id)?;
        let data = data_handle.get()?;
        let mut guard = data.lock_write()?;
        let (typed_crdt, ctx) = guard.get_context_and_node_mut()?;
        let Some(TypedCrdtMut::Register(mut register)) = typed_crdt else {
            Err(runtime_exception(format!(
                "Expected Register but found {}",
                type_string(typed_crdt)
            )))?
        };
        let value = env.convert_byte_array(value)?;
        register.set(ctx, value)?;
        Ok(())
    })
}

#[jni_method(package = "com.google.android.submerge", class = "SubmergeRegister")]
extern "system" fn nativeGet<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    data_handle_id: jlong,
) -> JByteArray<'local> {
    try_throwable_submerge(&mut env, |env| {
        let data_handle = SubmergeDataTypeHandle::try_from(data_handle_id)?;
        let data = data_handle.get()?;
        let mut guard = data.lock_write()?;
        let (typed_crdt, _) = guard.get_context_and_node_mut()?;
        let Some(TypedCrdtMut::Register(register)) = typed_crdt else {
            Err(runtime_exception(format!(
                "Expected Register but found {}",
                type_string(typed_crdt)
            )))?
        };
        Ok(match register.get() {
            Some(bytes) => env.byte_array_from_slice(bytes)?,
            None => JByteArray::default(), // Default is null
        })
    })
}

#[jni_method(package = "com.google.android.submerge", class = "SubmergeRegister")]
extern "system" fn nativeGetAll<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    data_handle_id: jlong,
) -> JObjectArray<'local> {
    try_throwable_submerge(&mut env, |env| {
        let data_handle = SubmergeDataTypeHandle::try_from(data_handle_id)?;
        let data = data_handle.get()?;
        let mut guard = data.lock_write()?;
        let (typed_crdt, _) = guard.get_context_and_node_mut()?;
        let Some(TypedCrdtMut::Register(register)) = typed_crdt else {
            Err(runtime_exception(format!(
                "Expected Register but found {}",
                type_string(typed_crdt)
            )))?
        };
        Ok(collect_to_jarray(
            env,
            "[B",
            register.get_all().into_iter(),
            |env, v| Ok(env.auto_local(env.byte_array_from_slice(v)?)),
        )?)
    })
}
