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

//! Java bindings for np_ffi_core.

// Named lifetimes are being used to match jni crate conventions.
// See: https://docs.rs/jni/latest/jni/struct.JNIEnv.html#lifetime-names
#![allow(clippy::needless_lifetimes)]

pub mod class;
#[cfg(feature = "testing")]
pub mod testing;

/// Helper to generate wrapper types for exception classes with no-arg constructors
macro_rules! exception_wrapper {
    ($(#[$docs:meta])* $name:ident, $cls:literal) => {
        $(#[$docs])*
        #[repr(transparent)]
        pub struct $name<'local>(pub JObject<'local>);

        impl<'local> $name<'local> {
            /// Create a new instance.
            pub fn construct(env: &mut JNIEnv<'local>) -> jni::errors::Result<Self> {
                static CLS: ClassDesc = ClassDesc::new($cls);
                pourover::call_constructor!(env, &CLS, "()V",).map(Self)
            }

            /// Create a new instance and throw it.
            pub fn throw_new(env: &mut JNIEnv<'local>) -> jni::errors::Result<()> {
                Self::construct(env)?.throw(env)
            }
        }

        impl<'local> $name<'local> {
            /// Throw this exception.
            pub fn throw<'env>(&self, env: &mut JNIEnv<'env>) -> jni::errors::Result<()> {
                env.throw(<&JThrowable>::from(self.0.as_ref()))
            }
        }

    }
}
pub(crate) use exception_wrapper;
