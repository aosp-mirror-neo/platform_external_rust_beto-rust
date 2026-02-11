// Copyright 2026 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
//
// you may not use this file except in compliance with the License.
//
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
//
// distributed under the License is distributed on an "AS IS" BASIS,
//
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//
// See the License for the specific language governing permissions and
//
// limitations under the License.
//
//

//! The codec library for encoding and decoding Nearby advertisements.
//!
//! This crate provides the `Codec` trait and implementations for handling different
//! advertisement formats and visibility scopes. The behavior, particularly regarding
//! encryption, is controlled by the `encrypted_identity` feature flag.

use crypto_provider::CryptoProvider;
use identity_manager::api::IdentityManager;
use std::sync::Arc;

/// The public API for the codec.
pub mod api;
/// Error types used by the codec.
pub mod errors;
/// Core types used for encoding and decoding.
pub mod types;

/// An implementation of the `Codec` trait.
pub mod codec_impl;
/// Decoding logic.
pub(crate) mod decode;
/// Encoding logic.
pub(crate) mod encode;

/// Retrieves an instance of the Codec.
pub fn get_codec<C>(identity_manager: Arc<dyn IdentityManager>) -> impl api::Codec
where
    C: CryptoProvider + Sync + Send + 'static,
{
    codec_impl::CodecImpl::<C>::new(identity_manager)
}

#[cfg(test)]
mod tests;
