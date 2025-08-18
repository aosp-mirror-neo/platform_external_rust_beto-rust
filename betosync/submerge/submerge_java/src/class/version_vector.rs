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

use std::cmp::Ordering;

use super::exceptions::try_throwable_submerge;
use distributed_time::vector_clock::VectorClock;
use handle_map::HandleLike;
use jni::{
    objects::{JByteArray, JClass, JObject},
    sys::{jint, jlong},
    JNIEnv,
};
use pourover::{desc::ClassDesc, exception::runtime_exception, jni_method};
use protobuf::Message;

use crate::utils::declare_handle;

declare_handle! {
    pub struct VersionVectorHandle for VectorClock<String>;
}

static VERSION_VECTOR_CLASS: ClassDesc =
    ClassDesc::new("com/google/android/submerge/VersionVector");

#[repr(transparent)]
#[derive(Default)]
pub struct VersionVectorJni<Obj>(pub Obj);

impl<'local> VersionVectorJni<JObject<'local>> {
    pub fn construct(
        env: &mut JNIEnv<'local>,
        version_vector: VersionVectorHandle,
    ) -> jni::errors::Result<Self> {
        pourover::call_constructor!(
            env,
            VERSION_VECTOR_CLASS,
            "(J)V",
            jlong::try_from(version_vector.handle_id)
                .expect("VersionVectorHandle's max_active_handles is 131072")
        )
        .map(Self)
    }
}

#[jni_method(package = "com.google.android.submerge", class = "VersionVector")]
extern "system" fn nativeClose<'local>(
    _env: JNIEnv<'local>,
    _cls: JClass<'local>,
    handle_id: jlong,
) {
    if let Ok(handle) = VersionVectorHandle::try_from(handle_id) {
        let _ = handle.deallocate();
    }
}

#[jni_method(package = "com.google.android.submerge", class = "VersionVector")]
extern "system" fn nativeToByteArray<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    handle_id: jlong,
) -> JByteArray<'local> {
    try_throwable_submerge(&mut env, |env| {
        let handle = VersionVectorHandle::try_from(handle_id)?;
        let version_vector = handle.get()?;
        let bytes = version_vector
            .to_transferable_proto()
            .write_to_bytes()
            .map_err(runtime_exception)?;
        Ok(env.byte_array_from_slice(&bytes)?)
    })
}

#[jni_method(package = "com.google.android.submerge", class = "VersionVector")]
extern "system" fn nativeFromByteArray<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    data: JByteArray<'local>,
) -> jlong {
    try_throwable_submerge(&mut env, |env| {
        let bytes = env.convert_byte_array(data)?;
        let proto =
            submerge_internal_proto::protos::submerge::VectorClock::parse_from_bytes(&bytes)
                .map_err(runtime_exception)?;
        let version_vector =
            VectorClock::from_transferable_proto(&proto).map_err(runtime_exception)?;
        Ok(VersionVectorHandle::allocate(|| version_vector)?.jlong())
    })
}

const OLDER: jint = -1;
const EQUAL: jint = 0;
const NEWER: jint = 1;
const CONCURRENT: jint = 2;

#[jni_method(package = "com.google.android.submerge", class = "VersionVector")]
extern "system" fn nativeCompare<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    handle_id: jlong,
    other_handle_id: jlong,
) -> jint {
    try_throwable_submerge(&mut env, |_| {
        let handle = VersionVectorHandle::try_from(handle_id)?;
        let self_version_vector = handle.get()?;
        let other_handle = VersionVectorHandle::try_from(other_handle_id)?;
        let other_version_vector = other_handle.get()?;
        Ok(
            match self_version_vector.partial_cmp(&other_version_vector) {
                Some(Ordering::Less) => OLDER,
                Some(Ordering::Equal) => EQUAL,
                Some(Ordering::Greater) => NEWER,
                None => CONCURRENT,
            },
        )
    })
}
