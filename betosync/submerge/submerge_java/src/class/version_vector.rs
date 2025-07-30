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

use distributed_time::vector_clock::VectorClock;
use handle_map::HandleLike;
use jni::{
    objects::{JClass, JObject},
    sys::jlong,
    JNIEnv,
};
use pourover::{desc::ClassDesc, jni_method};

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
