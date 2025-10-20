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

use crdt::{set::GenerationExhausted, vector_data::VersionOverflow};
use distributed_time::TimestampOverflow;
use handle_map::{HandleMapFullError, HandleNotPresentError};
use jni::{objects::JThrowable, JNIEnv};
use pourover::{
    desc::ClassDesc,
    exception::{runtime_exception, ThrowableJniError, ThrowableJniResultExt},
};

use super::version_vector::{VersionVectorHandle, VersionVectorJni};

pub fn ownership_exception(msg: impl Into<String>) -> jni::errors::Exception {
    jni::errors::Exception {
        class: "com/google/android/submerge/OwnershipException".into(),
        msg: msg.into(),
    }
}

pub fn generation_exhausted_exception(msg: impl Into<String>) -> jni::errors::Exception {
    jni::errors::Exception {
        class: "com/google/android/submerge/GenerationExhaustedException".into(),
        msg: msg.into(),
    }
}

pub fn version_overflow_exception(msg: impl Into<String>) -> jni::errors::Exception {
    jni::errors::Exception {
        class: "com/google/android/submerge/VersionOverflowException".into(),
        msg: msg.into(),
    }
}

pub fn timestamp_overflow_exception(msg: impl Into<String>) -> jni::errors::Exception {
    jni::errors::Exception {
        class: "com/google/android/submerge/TimestampOverflowException".into(),
        msg: msg.into(),
    }
}

pub fn transaction_exception(msg: impl Into<String>) -> jni::errors::Exception {
    jni::errors::Exception {
        class: "com/google/android/submerge/TransactionException".into(),
        msg: msg.into(),
    }
}

pub fn merge_exception(msg: impl Into<String>) -> jni::errors::Exception {
    jni::errors::Exception {
        class: "com/google/android/submerge/MergeException".into(),
        msg: msg.into(),
    }
}

static TRANSACTION_EXCEPTION_CLASS: ClassDesc =
    ClassDesc::new("com/google/android/submerge/TransactionException");

pub fn transaction_exception_with_cause<'local>(
    env: &mut JNIEnv<'local>,
    msg: impl Into<String>,
    cause: &JThrowable<'local>,
) -> Result<JThrowable<'local>, jni::errors::Error> {
    pourover::call_constructor!(
        env,
        TRANSACTION_EXCEPTION_CLASS,
        "(Ljava/lang/String;Ljava/lang/Throwable;)V",
        env.new_string(msg.into())?.as_ref(),
        cause
    )
    .map(|obj| obj.into())
}

static OLDER_BASE_NEEDED_EXCEPTION_CLASS: ClassDesc =
    ClassDesc::new("com/google/android/submerge/OlderBaseNeededException");

pub fn older_base_needed_exception<'local>(
    env: &mut JNIEnv<'local>,
    version_handle: VersionVectorHandle,
) -> Result<JThrowable<'local>, jni::errors::Error> {
    let version_java = VersionVectorJni::construct(env, version_handle)?;
    pourover::call_constructor!(
        env,
        OLDER_BASE_NEEDED_EXCEPTION_CLASS,
        "(Lcom/google/android/submerge/VersionVector;)V",
        &version_java.0
    )
    .map(|obj| obj.into())
}

pub struct SubmergeJavaError<'local>(ThrowableJniError<'local>);

impl From<HandleNotPresentError> for SubmergeJavaError<'_> {
    fn from(_: HandleNotPresentError) -> Self {
        Self(ThrowableJniError::from(runtime_exception(
            "Handle not present",
        )))
    }
}

impl From<HandleMapFullError> for SubmergeJavaError<'_> {
    fn from(_: HandleMapFullError) -> Self {
        Self(ThrowableJniError::from(runtime_exception(
            "Handle map full",
        )))
    }
}

impl From<jni::errors::Exception> for SubmergeJavaError<'_> {
    fn from(exception: jni::errors::Exception) -> Self {
        Self(ThrowableJniError::from(exception))
    }
}

impl From<jni::errors::Error> for SubmergeJavaError<'_> {
    fn from(error: jni::errors::Error) -> Self {
        Self(ThrowableJniError::from(error))
    }
}

impl<'local> From<JThrowable<'local>> for SubmergeJavaError<'local> {
    fn from(throwable: JThrowable<'local>) -> Self {
        Self(ThrowableJniError::from(throwable))
    }
}

impl From<TimestampOverflow> for SubmergeJavaError<'_> {
    fn from(_: TimestampOverflow) -> Self {
        Self(ThrowableJniError::from(timestamp_overflow_exception(
            "Timestamp overflow",
        )))
    }
}

impl From<GenerationExhausted> for SubmergeJavaError<'_> {
    fn from(_: GenerationExhausted) -> Self {
        Self(ThrowableJniError::from(generation_exhausted_exception(
            "Generation number exhausted",
        )))
    }
}

impl From<VersionOverflow> for SubmergeJavaError<'_> {
    fn from(_: VersionOverflow) -> Self {
        Self(ThrowableJniError::from(version_overflow_exception(
            "Version overflow",
        )))
    }
}

impl<'local> From<ThrowableJniError<'local>> for SubmergeJavaError<'local> {
    fn from(value: ThrowableJniError<'local>) -> Self {
        Self(value)
    }
}

impl<'local> From<SubmergeJavaError<'local>> for ThrowableJniError<'local> {
    fn from(value: SubmergeJavaError<'local>) -> Self {
        value.0
    }
}

pub fn try_throwable_submerge<'local, R: Default>(
    env: &mut JNIEnv<'local>,
    func: impl FnOnce(&mut JNIEnv<'local>) -> Result<R, SubmergeJavaError<'local>>,
) -> R {
    func(env)
        .map_err(ThrowableJniError::from)
        .unwrap_or_throw(env)
}
