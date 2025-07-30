// Copyright 2023 Google LLC
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

use crate::exception_wrapper;
use jni::{
    objects::{JObject, JThrowable},
    JNIEnv,
};
use np_adv::AdvDeserializationError;
use pourover::desc::ClassDesc;

exception_wrapper!(
    /// Rust representation of `class DeserializationException.InvalidHeaderException`.
    InvalidHeaderException, "com/google/android/nearby/presence/rust/DeserializationException$InvalidHeaderException");

exception_wrapper!(
    /// Rust representation of `class DeserializationException.InvalidFormatException`.
    InvalidFormatException, "com/google/android/nearby/presence/rust/DeserializationException$InvalidFormatException");

/// Allow AdvDeserializationError to be thrown as a Java exception.
impl super::ToJavaException for AdvDeserializationError {
    fn throw_java_exception<'env>(&self, env: &mut JNIEnv<'env>) -> jni::errors::Result<()> {
        match *self {
            AdvDeserializationError::VersionHeaderParseError => {
                InvalidHeaderException::throw_new(env)
            }
            AdvDeserializationError::ParseError { .. } => InvalidFormatException::throw_new(env),
        }
    }
}
