// Copyright 2022 Google LLC
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

//! Serialization and deserialization for v0 (legacy) and v1 (extended) Nearby Presence
//! advertisements.
//!
//! See `tests/examples_v0.rs` and `tests/examples_v1.rs` for some tests that show common
//! deserialization scenarios.

#![no_std]
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

#[cfg(any(test, feature = "alloc"))]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

use core::fmt;
/// Re-export of the `strum` crate.
pub use strum;
/// Re-export of `tinyvec::ArrayVec`.
pub use tinyvec::ArrayVec;

use crate::buffer::AdvBuffer;
use crate::credential::matched::MatchedCredential;
use crate::extended::{
    deserialize::{deser_decrypt_v1, V1AdvertisementContents},
    MIN_BUFFER_CAPACITY,
};
use crate::{
    credential::book::CredentialBook,
    header::UpVersionHeader,
    legacy::{deser_decrypt_v0, V0AdvertisementContents},
};
use core::fmt::Debug;
use crypto_provider::CryptoProvider;
use deserialization_arena::DeserializationArena;
use legacy::data_elements::DataElementDeserializeError;

#[cfg(test)]
mod tests;

mod array_vec;
/// Buffer types for advertisement serialization and deserialization.
pub mod buffer;
/// Types for handling credentials.
pub mod credential;
/// Arena for zero-copy deserialization.
pub mod deserialization_arena;
/// Types for V1 (extended) advertisements.
pub mod extended;
/// Advertisement filtering logic.
pub mod filter;
mod header;
mod helpers;
/// Types for V0 (legacy) advertisements.
pub mod legacy;
/// Data shared between advertisement sections.
pub mod shared_data;

/// Canonical form of UP's service UUID.
///
/// Note that UUIDs are encoded in BT frames in little-endian order, so these bytes may need to be
/// reversed depending on the host BT API.
pub const UP_SVC_UUID: [u8; 2] = [0xFC, 0xF1];

/// Creates a default advertisement buffer allocated on the heap.
pub fn default_vec_buffer() -> buffer::VecBuffer {
    buffer::VecBuffer::new(MIN_BUFFER_CAPACITY)
}

/// Creates an advertisement buffer allocated on the stack.
pub fn stack_buffer<const N: usize>() -> buffer::StackBuffer<N> {
    buffer::StackBuffer::<N>::new()
}

/// Parse, deserialize, decrypt, and validate a complete UP advertisement (the entire contents of
/// the service data for the UP UUID).
pub fn deserialize_advertisement<'adv, 'cred, C, Buf, P>(
    arena: DeserializationArena<'adv, Buf>,
    adv: &'adv [u8],
    cred_book: &'cred C,
) -> Result<DeserializedAdvertisement<'adv, C::Matched>, AdvDeserializationError>
where
    C: CredentialBook<'cred>,
    Buf: AdvBuffer,
    P: CryptoProvider,
{
    let (remaining, header) = UpVersionHeader::parse(adv)
        .map_err(|_e| AdvDeserializationError::VersionHeaderParseError)?;
    match header {
        UpVersionHeader::V0(encoding) => deser_decrypt_v0::<C, P>(encoding, cred_book, remaining)
            .map(DeserializedAdvertisement::V0),
        UpVersionHeader::V1(header) => {
            deser_decrypt_v1::<C, Buf, P>(arena, cred_book, remaining, header)
                .map(DeserializedAdvertisement::V1)
        }
    }
}

/// A deserialized UP advertisement.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, PartialEq, Eq)]
pub enum DeserializedAdvertisement<'adv, M: MatchedCredential> {
    /// A V0 advertisement.
    V0(V0AdvertisementContents<'adv, M>),
    /// A V1 advertisement.
    V1(V1AdvertisementContents<'adv, M>),
}

impl<'adv, M: MatchedCredential> DeserializedAdvertisement<'adv, M> {
    /// Attempts to cast this deserialized advertisement into the `V0AdvertisementContents`
    /// variant. If the underlying advertisement is not V0, this will instead return `None`.
    pub fn into_v0(self) -> Option<V0AdvertisementContents<'adv, M>> {
        match self {
            Self::V0(x) => Some(x),
            _ => None,
        }
    }
    /// Attempts to cast this deserialized advertisement into the `V1AdvertisementContents`
    /// variant. If the underlying advertisement is not V1, this will instead return `None`.
    pub fn into_v1(self) -> Option<V1AdvertisementContents<'adv, M>> {
        match self {
            Self::V1(x) => Some(x),
            _ => None,
        }
    }
}

/// Errors that can occur during advertisement deserialization.
#[derive(PartialEq)]
pub enum AdvDeserializationError {
    /// The advertisement header could not be parsed
    VersionHeaderParseError,
    /// The advertisement content could not be parsed
    ParseError {
        /// Potentially hazardous details about deserialization errors. Read the documentation for
        /// [AdvDeserializationErrorDetailsHazmat] before using this field.
        details_hazmat: AdvDeserializationErrorDetailsHazmat,
    },
}

impl Debug for AdvDeserializationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdvDeserializationError::VersionHeaderParseError => {
                write!(f, "VersionHeaderParseError")
            }
            AdvDeserializationError::ParseError { .. } => write!(f, "ParseError"),
        }
    }
}

impl fmt::Display for AdvDeserializationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdvDeserializationError::VersionHeaderParseError => {
                write!(f, "VersionHeaderParseError")
            }
            AdvDeserializationError::ParseError { .. } => write!(f, "ParseError"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AdvDeserializationError {}

/// Potentially hazardous details about deserialization errors. These error information can
/// potentially expose side-channel information about the plaintext of the advertisements and/or
/// the keys used to decrypt them. For any place that you avoid exposing the keys directly
/// (e.g. across FFIs, print to log, etc), avoid exposing these error details as well.
#[derive(PartialEq)]
pub enum AdvDeserializationErrorDetailsHazmat {
    /// Parsing the overall advertisement or DE structure failed
    AdvertisementDeserializeError,
    /// Deserializing an individual DE from its DE contents failed
    V0DataElementDeserializeError(DataElementDeserializeError),
    /// Non-identity DE contents must not be empty
    NoPublicDataElements,
}

impl From<legacy::deserialize::AdvDeserializeError> for AdvDeserializationError {
    fn from(err: legacy::deserialize::AdvDeserializeError) -> Self {
        match err {
            legacy::deserialize::AdvDeserializeError::NoDataElements => {
                AdvDeserializationError::ParseError {
                    details_hazmat: AdvDeserializationErrorDetailsHazmat::NoPublicDataElements,
                }
            }
            legacy::deserialize::AdvDeserializeError::InvalidStructure => {
                AdvDeserializationError::ParseError {
                    details_hazmat:
                        AdvDeserializationErrorDetailsHazmat::AdvertisementDeserializeError,
                }
            }
        }
    }
}

// trivial Debug/Display that won't leak anything
impl Debug for AdvDeserializationErrorDetailsHazmat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AdvDeserializationErrorDetailsHazmat")
    }
}

impl fmt::Display for AdvDeserializationErrorDetailsHazmat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AdvDeserializationErrorDetailsHazmat")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for AdvDeserializationErrorDetailsHazmat {}

/// DE length is out of range (e.g. > 4 bits for encoded V0, > max DE size for actual V0, >127 for
/// V1) or invalid for the relevant DE type.
#[derive(Debug, PartialEq, Eq)]
pub struct DeLengthOutOfRange;

pub(crate) mod private {
    /// A marker trait to prevent other crates from implementing traits that
    /// are intended to be only implemented internally.
    pub trait Sealed {}
}
