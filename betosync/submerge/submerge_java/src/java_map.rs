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

#![allow(unused, dead_code)]

use jni::{objects::JObject, JNIEnv};
use pourover::desc::ClassDesc;

static JAVA_HASH_MAP_CLASS: ClassDesc = ClassDesc::new("java/util/HashMap");

#[repr(transparent)]
#[derive(Default)]
pub(crate) struct HashMapJni<Obj>(pub Obj);

impl<'local> HashMapJni<JObject<'local>> {
    pub fn construct(
        env: &mut JNIEnv<'local>,
        initial_capacity: i32,
    ) -> Result<Self, jni::errors::Error> {
        pourover::call_constructor!(env, JAVA_HASH_MAP_CLASS, "(I)V", initial_capacity).map(Self)
    }

    pub fn put(
        &self,
        env: &mut JNIEnv<'local>,
        key: impl AsRef<JObject<'local>>,
        value: impl AsRef<JObject<'local>>,
    ) -> Result<JObject<'local>, jni::errors::Error> {
        pourover::call_method!(
            env,
            JAVA_HASH_MAP_CLASS,
            "put",
            "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
            &self.0,
            key.as_ref(),
            value.as_ref()
        )
    }

    pub fn get(
        &self,
        env: &mut JNIEnv<'local>,
        key: impl AsRef<JObject<'local>>,
    ) -> Result<JObject<'local>, jni::errors::Error> {
        pourover::call_method!(
            env,
            JAVA_HASH_MAP_CLASS,
            "get",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            &self.0,
            key.as_ref(),
        )
    }
}
