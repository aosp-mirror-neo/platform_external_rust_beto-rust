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

use distributed_time::WallTimestampProvider;
use jni::{
    objects::{GlobalRef, JObject},
    sys::jlong,
    JNIEnv, JavaVM,
};
use pourover::{desc::ClassDesc, exception::runtime_exception};

/// A wrapper struct around a Java-implemented timestamp provider that implements
/// [`WallTimestampProvider`].
#[derive(Debug, Clone)]
pub struct JavaTimestampProvider {
    jobject: TimestampProviderJni<GlobalRef>,
    vm: Arc<JavaVM>,
}

impl JavaTimestampProvider {
    pub fn new<'local>(
        env: &mut JNIEnv<'local>,
        timestamp_provider: TimestampProviderJni<JObject<'local>>,
    ) -> Result<Self, jni::errors::Error> {
        Ok(Self {
            jobject: timestamp_provider.to_global_ref(env)?,
            vm: Arc::new(env.get_java_vm()?),
        })
    }
}

impl WallTimestampProvider for JavaTimestampProvider {
    fn now(&self) -> u64 {
        let mut env = self
            .vm
            .get_env()
            .expect("Unable to get JVM env in infallible `now()`");
        match self.jobject.now(&mut env) {
            Ok(v) => u64::try_from(v).unwrap_or(0),
            Err(jni::errors::Error::JavaException) => 0,
            Err(e) => {
                env.throw(runtime_exception(e))
                    .expect("Failed to throw Java exception");
                // Return value is ignored by the JVM since an exception was thrown. Anyhow, using
                // an incorrect timestamp won't affect the data consistency.
                0
            }
        }
    }
}

/// A transparent wrapper over a Java implemented `TimestampProvider`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct TimestampProviderJni<Obj>(Obj);

static TIMESTAMP_PROVIDER_CLASS: ClassDesc =
    ClassDesc::new("com/google/android/submerge/TimestampProvider");

impl<'local, O: AsRef<JObject<'local>>> TimestampProviderJni<O> {
    pub fn to_global_ref(
        &self,
        env: &mut JNIEnv<'_>,
    ) -> Result<TimestampProviderJni<GlobalRef>, jni::errors::Error> {
        Ok(TimestampProviderJni(env.new_global_ref(&self.0)?))
    }

    pub fn now(&self, env: &mut JNIEnv<'_>) -> Result<jlong, jni::errors::Error> {
        let timestamp =
            pourover::call_method!(env, TIMESTAMP_PROVIDER_CLASS, "now", "()J", self.0.as_ref(),)?;
        Ok(timestamp)
    }
}
