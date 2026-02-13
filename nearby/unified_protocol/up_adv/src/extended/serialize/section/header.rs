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

//! Section headers for serialization.

use crate::extended::{
    salt::ShortV1Salt, V1IdentityToken, V1_ENCODING_ENCRYPTED, V1_ENCODING_ENCRYPTED_SHORT,
    V1_ENCODING_UNENCRYPTED,
};
use tinyvec::ArrayVec;

/// Length of a section header without its trailing length byte: `format || salt || token`
pub(crate) const SECTION_HEADER_MAX_LEN: usize = 1 + 16 + 16;

/// Assembles the header bytes for a section before the section length, before postprocessing by
/// a [SectionEncoder](crate::extended::serialize::section::SectionEncoder).
///
/// Does not include the overall UP header byte that defines the adv version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionHeader {
    /// The section header
    /// Invariant: always has at least 1 byte
    pub(crate) header_bytes: ArrayVec<[u8; SECTION_HEADER_MAX_LEN]>,
}

impl SectionHeader {
    pub(crate) fn unencrypted() -> Self {
        let mut header_bytes = ArrayVec::new();
        header_bytes.push(V1_ENCODING_UNENCRYPTED.0);
        Self { header_bytes }
    }

    pub(crate) fn encrypted_mic_encrypted(token: &V1IdentityToken) -> Self {
        let mut header_bytes = ArrayVec::new();
        header_bytes.push(V1_ENCODING_ENCRYPTED.0);
        header_bytes.extend_from_slice(token.as_slice());

        Self { header_bytes }
    }

    pub(crate) fn encrypted_mic_short_salt(salt: ShortV1Salt, token: &V1IdentityToken) -> Self {
        let mut header_bytes = ArrayVec::new();
        header_bytes.push(V1_ENCODING_ENCRYPTED_SHORT.0);
        header_bytes.extend_from_slice(salt.bytes());
        header_bytes.extend_from_slice(token.as_slice());

        Self { header_bytes }
    }

    pub(crate) fn as_slice(&self) -> &[u8] {
        self.header_bytes.as_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extended::{
        salt::ShortV1Salt, V1IdentityToken, V1_ENCODING_ENCRYPTED, V1_ENCODING_ENCRYPTED_SHORT,
        V1_ENCODING_UNENCRYPTED,
    };
    use alloc::vec;

    #[test]
    fn test_unencrypted() {
        let header = SectionHeader::unencrypted();
        assert_eq!(header.as_slice(), &[V1_ENCODING_UNENCRYPTED.0]);
    }

    #[test]
    fn test_encrypted_mic_encrypted() {
        let token_bytes = [0x11; 16];
        let token = V1IdentityToken::from(token_bytes);
        let header = SectionHeader::encrypted_mic_encrypted(&token);

        let mut expected = vec![V1_ENCODING_ENCRYPTED.0];
        expected.extend_from_slice(&token_bytes);

        assert_eq!(header.as_slice(), expected.as_slice());
    }

    #[test]
    fn test_encrypted_mic_short_salt() {
        let salt_bytes = [0x22, 0x33];
        let salt = ShortV1Salt::from(salt_bytes);
        let token_bytes = [0x44; 16];
        let token = V1IdentityToken::from(token_bytes);

        let header = SectionHeader::encrypted_mic_short_salt(salt, &token);

        // Note: The implementation currently uses V1_ENCODING_ENCRYPTED for short salt
        let mut expected = vec![V1_ENCODING_ENCRYPTED_SHORT.0];
        expected.extend_from_slice(&salt_bytes);
        expected.extend_from_slice(&token_bytes);

        assert_eq!(header.as_slice(), expected.as_slice());
    }
}
