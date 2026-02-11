// Copyright 2024 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![allow(clippy::unwrap_used)]

use super::*;

#[cfg(test)]
mod mic_decrypt_tests;

#[cfg(test)]
mod coverage_gaming;

/// An error when attempting to resolve an identity and then
/// attempt to deserialize an encrypted advertisement.
///
/// This should not be exposed publicly, since it's too
/// detailed.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum IdentityResolutionOrDeserializationError<V: VerificationError> {
    /// Failed to match the encrypted adv to an identity
    IdentityMatchingError,
    /// Failed to deserialize the encrypted adv after matching the identity
    DeserializationError(DeserializationError<V>),
}

impl<V: VerificationError> From<DeserializationError<V>>
    for IdentityResolutionOrDeserializationError<V>
{
    fn from(deserialization_error: DeserializationError<V>) -> Self {
        Self::DeserializationError(deserialization_error)
    }
}

impl<V: VerificationError> From<V> for IdentityResolutionOrDeserializationError<V> {
    fn from(verification_error: V) -> Self {
        Self::DeserializationError(DeserializationError::VerificationError(verification_error))
    }
}
