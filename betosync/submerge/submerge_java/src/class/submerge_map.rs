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

use crdt::{
    lww_map::{LwwMapRead, LwwMapWrite},
    typed_crdt::TypedCrdtMut,
};
use handle_map::HandleLike;
use jni::{
    objects::{JClass, JObjectArray, JString},
    sys::jlong,
    JNIEnv,
};
use pourover::{exception::runtime_exception, jni_method};

use crate::{
    class::submerge_data_type::{CrdtType, JavaSubmergeDataType, SubmergeDataTypeHandle, TreeRoot},
    utils::collect_to_jarray,
};

use super::{
    exceptions::{ownership_exception, try_throwable_submerge},
    submerge_data_type::type_string,
};

#[jni_method(package = "com.google.android.submerge", class = "SubmergeMap")]
extern "system" fn nativeGetChild<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    data_handle_id: jlong,
    key: JString<'local>,
) -> jlong {
    try_throwable_submerge(&mut env, |env| {
        let new_data_type = {
            // Create a scope for the SubmergeDataTypeHandle shard locks. They must be released
            // before trying to allocate a new SubmergeDataTypeHandle for the return value.
            let data_handle = SubmergeDataTypeHandle::try_from(data_handle_id)?;
            let data = data_handle.get()?;
            let mut guard = data.lock_write()?;
            let (typed_crdt, _) = guard.get_context_and_node_mut()?;
            let key = env.get_string(&key).map(String::from)?;
            let Some(TypedCrdtMut::Map(map)) = typed_crdt else {
                Err(runtime_exception(format!(
                    "Expected Map but found {}",
                    type_string(typed_crdt)
                )))?
            };
            let crdt_type = map.get_value(&key).map(CrdtType::from);
            let path = [&data.path, [key].as_slice()].concat();
            let root_handle = data.root_handle.clone();
            JavaSubmergeDataType {
                path,
                crdt_type,
                root_handle,
            }
        };
        Ok(SubmergeDataTypeHandle::allocate(|| new_data_type)?.jlong())
    })
}

#[jni_method(package = "com.google.android.submerge", class = "SubmergeMap")]
extern "system" fn nativeSetChild<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    data_handle_id: jlong,
    key: JString<'local>,
    child_handle_id: jlong,
) -> jlong {
    try_throwable_submerge(&mut env, |env| {
        let child_data_type = SubmergeDataTypeHandle::try_from(child_handle_id)?.deallocate()?;
        let new_data = {
            // Create a scope for the SubmergeDataTypeHandle shard locks. They must be released
            // before trying to allocate a new SubmergeDataTypeHandle for the return value.
            let data_handle = SubmergeDataTypeHandle::try_from(data_handle_id)?;
            let data = data_handle.get()?;
            if !matches!(&data.root_handle, TreeRoot::Document(_)) {
                return Err(ownership_exception(
                    "Map must be added to a document before calling Map.setChild",
                ))?;
            }
            let mut guard = data.lock_write()?;
            let (typed_crdt, ctx) = guard.get_context_and_node_mut()?;
            let Some(TypedCrdtMut::Map(mut map)) = typed_crdt else {
                Err(runtime_exception(format!(
                    "Expected Map but found {}",
                    type_string(typed_crdt)
                )))?
            };
            let key = env.get_string(&key).map(String::from)?;
            let TreeRoot::Detached { root: child, .. } = child_data_type.root_handle else {
                Err(ownership_exception("Cannot set child which is not owned"))?
            };
            let child = Arc::into_inner(child)
                .ok_or_else(|| ownership_exception("Outstanding refs, cannot take ownership"))?
                .into_inner();
            if let Some(value) = child {
                map.set_value(ctx, key.clone(), value)?;
            } else {
                map.remove_value(ctx, key.clone())?;
            }
            JavaSubmergeDataType {
                path: [data.path.as_slice(), [key].as_slice()].concat(),
                crdt_type: child_data_type.crdt_type,
                root_handle: data.root_handle.clone(),
            }
        };
        Ok(SubmergeDataTypeHandle::allocate(|| new_data)?.jlong())
    })
}

#[jni_method(package = "com.google.android.submerge", class = "SubmergeMap")]
extern "system" fn nativeRemoveChild<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    data_handle_id: jlong,
    key: JString<'local>,
) {
    try_throwable_submerge(&mut env, |env| {
        let data_handle = SubmergeDataTypeHandle::try_from(data_handle_id)?;
        let data = data_handle.get()?;
        let mut guard = data.lock_write()?;
        let (typed_crdt, ctx) = guard.get_context_and_node_mut()?;
        let Some(TypedCrdtMut::Map(mut map)) = typed_crdt else {
            Err(runtime_exception(format!(
                "Expected Map but found {}",
                type_string(typed_crdt)
            )))?
        };
        let key = env.get_string(&key).map(String::from)?;
        map.remove_value(ctx, key)?;
        Ok(())
    })
}

#[jni_method(package = "com.google.android.submerge", class = "SubmergeMap")]
extern "system" fn nativeKeySet<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    data_handle_id: jlong,
) -> JObjectArray<'local> {
    try_throwable_submerge(&mut env, |env| {
        let data_handle = SubmergeDataTypeHandle::try_from(data_handle_id)?;
        let data = data_handle.get()?;
        let mut guard = data.lock_write()?;
        let (typed_crdt, _) = guard.get_context_and_node_mut()?;
        let Some(TypedCrdtMut::Map(map)) = typed_crdt else {
            Err(runtime_exception(format!(
                "Expected Map but found {}",
                type_string(typed_crdt)
            )))?
        };
        Ok(collect_to_jarray(
            env,
            "java/lang/String",
            map.keys()
                .filter(|key| map.get_value(key).is_some())
                .collect::<Vec<_>>()
                .into_iter(),
            |env, item| Ok(env.auto_local(env.new_string(item)?)),
        )?)
    })
}
