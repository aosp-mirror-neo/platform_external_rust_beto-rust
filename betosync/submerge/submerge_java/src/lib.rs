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

//! Java bindings for submerge.
//!
//! This crate contains the JNI glue code that implements the corresponding Java `native` methods
//! and forwards them to the [`submerge`] or [`crdt`] crates.
//!
//! This crate is not intended to be used by any other crates; only directly built as a dynamic
//! library (e.g. .so file) to be loaded by System.loadLibrary in Java.
//!
//! ## Maintenance notes
//!
//! Since going through the JNI means we lose the help of the type system, there are some patterns
//! we follow to help us organize things:
//! 1. Handles are used to isolate access to Rust-owned data. See [`handle_map`] for more.
//! 2. The types we expose (possibly via the handle indirection) may need to contain additional data
//!    for Java's APIs. In those cases, those wrapper structs are prefixed with "Java-". Similarly,
//!    the "Java-" prefix is added for type aliases on Rust types that are exposed through JNI.
//! 3. For Java types that we need to access in Rust, the suffix "-Jni" is added. These are wrappers
//!    around `JObject`s.
//! 4. The handles are `#[repr(transparent)]` and holds a `u64` which are implicitly transmuted into
//!    Java `long`s when it goes through JNI. In Java we rely on annotations to help document the
//!    types. Currently there are no automated tooling setup to check that those type annotations
//!    are respected in code.

// Named lifetimes are being used to match jni crate conventions.
// See: https://docs.rs/jni/latest/jni/struct.JNIEnv.html#lifetime-names
#![allow(clippy::needless_lifetimes)]
// Trivial casts needed for FFI and more readable than creating new bindings
#![allow(trivial_casts)]

mod class;
mod java_map;
mod utils;
