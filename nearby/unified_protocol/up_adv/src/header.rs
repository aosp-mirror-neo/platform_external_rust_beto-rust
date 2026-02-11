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

//! UP version header (the first byte) support.
//!
//! The version header byte is 3 bits of version followed by 5 reserved bits.
//!
//! For V0 (`0b000`), the first 3 of the reserved bits identify the encoding
//! scheme used, and the last 2 are still reserved.

use nom::{bytes::complete as bytes_complete, combinator, number};

// 3-bit versions (high 3 bits in version header)
const PROTOCOL_VERSION_LEGACY: u8 = 0;
const PROTOCOL_VERSION_EXTENDED: u8 = 1;

// 3-bit encoding ids (3 bits after version, leaving 2 reserved bits)
const V0_ENCODING_SCHEME_ID_UNENCRYPTED: u8 = 0;
const V0_ENCODING_SCHEME_ID_LDT: u8 = 1;

/// Represents the parsed first byte of the broadcast.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum BroadcastVersion {
    V0(V0Encoding),
    V1 { has_fixed_identity_hash: bool, has_root_salt: bool },
    Unsupported { version_number: u8, version_specific: u8 },
}

impl BroadcastVersion {
    const VERSION_NUMBER_SHIFT: u8 = 5;
    const VERSION_SPECIFIC_MASK: u8 = 0x1F; // Lower 5 bits

    // V1 Flags (within the 5 version_specific bits)
    const V1_FLAG_FIXED_IDENTITY_HASH: u8 = 0x08;
    const V1_FLAG_ROOT_SALT: u8 = 0x10;

    /// Parses the version byte.
    pub fn from_byte(byte: u8) -> Self {
        let version_number = byte >> Self::VERSION_NUMBER_SHIFT;
        let version_specific = byte & Self::VERSION_SPECIFIC_MASK;

        match version_number {
            PROTOCOL_VERSION_LEGACY => match byte {
                VERSION_HEADER_V0_UNENCRYPTED => BroadcastVersion::V0(V0Encoding::Unencrypted),
                VERSION_HEADER_V0_LDT => BroadcastVersion::V0(V0Encoding::Ldt),
                _ => BroadcastVersion::Unsupported { version_number, version_specific },
            },
            PROTOCOL_VERSION_EXTENDED => {
                // Check for any reserved bits being set
                if (version_specific
                    & !(Self::V1_FLAG_FIXED_IDENTITY_HASH | Self::V1_FLAG_ROOT_SALT))
                    != 0
                {
                    BroadcastVersion::Unsupported { version_number, version_specific }
                } else {
                    BroadcastVersion::V1 {
                        has_fixed_identity_hash: (version_specific
                            & Self::V1_FLAG_FIXED_IDENTITY_HASH)
                            != 0,
                        has_root_salt: (version_specific & Self::V1_FLAG_ROOT_SALT) != 0,
                    }
                }
            }
            _ => BroadcastVersion::Unsupported { version_number, version_specific },
        }
    }

    /// Encodes to a byte.
    pub fn to_byte(self) -> u8 {
        match self {
            BroadcastVersion::V0(V0Encoding::Unencrypted) => VERSION_HEADER_V0_UNENCRYPTED,
            BroadcastVersion::V0(V0Encoding::Ldt) => VERSION_HEADER_V0_LDT,
            BroadcastVersion::V1 { has_fixed_identity_hash, has_root_salt } => {
                let mut specific = 0u8;
                if has_fixed_identity_hash {
                    specific |= BroadcastVersion::V1_FLAG_FIXED_IDENTITY_HASH;
                }
                if has_root_salt {
                    specific |= BroadcastVersion::V1_FLAG_ROOT_SALT;
                }
                (PROTOCOL_VERSION_EXTENDED << BroadcastVersion::VERSION_NUMBER_SHIFT) | specific
            }
            BroadcastVersion::Unsupported { version_number, version_specific } => {
                (version_number << BroadcastVersion::VERSION_NUMBER_SHIFT) | version_specific
            }
        }
    }
}

/// Version header byte for V1
pub const VERSION_HEADER_V1: u8 = PROTOCOL_VERSION_EXTENDED << 5;
const V1_HEADER_HASH_LEN: usize = 3;
pub(crate) const V1_HEADER_SALT_LEN: usize = 16;

/// Version header byte for V0 with the unencrypted encoding
pub const VERSION_HEADER_V0_UNENCRYPTED: u8 =
    (PROTOCOL_VERSION_LEGACY << 5) | (V0_ENCODING_SCHEME_ID_UNENCRYPTED << 2);

/// Version header byte for V0 with the LDT encoding
pub const VERSION_HEADER_V0_LDT: u8 =
    (PROTOCOL_VERSION_LEGACY << 5) | (V0_ENCODING_SCHEME_ID_LDT << 2);

/// The first byte in the UP svc data. It defines which version of the protocol
/// is used for the rest of the svc data.
#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) enum UpVersionHeader {
    V0(V0Encoding),
    V1(V1AdvHeader),
}

impl UpVersionHeader {
    /// Parse a UP advertisement header.
    ///
    /// This can be used on all versions of advertisements since it's the header that determines the
    /// version.
    ///
    /// Returns a `nom::IResult` with the parsed header and the remaining bytes of the advertisement.
    pub(crate) fn parse(adv: &[u8]) -> nom::IResult<&[u8], Self> {
        let (input, header_byte) = number::complete::u8(adv)?;
        let version = BroadcastVersion::from_byte(header_byte);

        match version {
            BroadcastVersion::V0(encoding) => Ok((input, UpVersionHeader::V0(encoding))),
            BroadcastVersion::V1 { has_fixed_identity_hash, has_root_salt } => {
                let (input, v1_header) =
                    V1AdvHeader::parse(version, has_fixed_identity_hash, has_root_salt, input)?;
                Ok((input, UpVersionHeader::V1(v1_header)))
            }
            BroadcastVersion::Unsupported { .. } => combinator::fail(adv),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum V0Encoding {
    Unencrypted,
    Ldt,
}

/// A parsed UP Version Header that indicates the V1 protocol is in use.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct V1AdvHeader {
    version: BroadcastVersion,
    fixed_identity_hash: Option<[u8; V1_HEADER_HASH_LEN]>,
    root_salt: Option<[u8; V1_HEADER_SALT_LEN]>,
}

impl V1AdvHeader {
    pub(crate) fn parse(
        version: BroadcastVersion,
        has_fixed_identity_hash: bool,
        has_root_salt: bool,
        input: &[u8],
    ) -> nom::IResult<&[u8], Self> {
        let (input, fixed_identity_hash) = combinator::cond(
            has_fixed_identity_hash,
            combinator::map(bytes_complete::take(V1_HEADER_HASH_LEN), |h: &[u8]| {
                h.try_into().expect("slice length checked by take")
            }),
        )(input)?;

        let (input, root_salt) = combinator::cond(
            has_root_salt,
            combinator::map(bytes_complete::take(V1_HEADER_SALT_LEN), |s: &[u8]| {
                s.try_into().expect("slice length checked by take")
            }),
        )(input)?;

        Ok((input, Self { version, fixed_identity_hash, root_salt }))
    }

    /// The version header byte
    pub(crate) fn contents(&self) -> u8 {
        self.version.to_byte()
    }

    /// The fixed identity hash, if present.
    #[allow(dead_code)]
    pub fn fixed_identity_hash(&self) -> Option<[u8; V1_HEADER_HASH_LEN]> {
        self.fixed_identity_hash
    }

    /// The root salt, if present.
    #[allow(dead_code)]
    pub fn root_salt(&self) -> Option<[u8; V1_HEADER_SALT_LEN]> {
        self.root_salt
    }

    #[cfg(any(test, feature = "devtools"))]
    pub(crate) fn new(
        header_byte: u8,
        fixed_identity_hash: Option<[u8; V1_HEADER_HASH_LEN]>,
        root_salt: Option<[u8; V1_HEADER_SALT_LEN]>,
    ) -> Self {
        Self { version: BroadcastVersion::from_byte(header_byte), fixed_identity_hash, root_salt }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    extern crate std;

    use nom::error;

    #[test]
    fn parse_header_v0_unencrypted() {
        let (_, header) = UpVersionHeader::parse(&[0x00]).unwrap();
        assert_eq!(UpVersionHeader::V0(V0Encoding::Unencrypted), header);
    }

    #[test]
    fn parse_header_v0_ldt() {
        let (_, header) = UpVersionHeader::parse(&[0x04]).unwrap();
        assert_eq!(UpVersionHeader::V0(V0Encoding::Ldt), header);
    }

    #[test]
    fn parse_header_v0_nonzero_invalid_encoding() {
        let input = &[0x08];
        assert_eq!(
            nom::Err::Error(error::Error { input: input.as_slice(), code: error::ErrorKind::Fail }),
            UpVersionHeader::parse(input).unwrap_err()
        );
    }

    #[test]
    fn parse_header_v0_nonzero_reserved() {
        let input = &[0x01];
        assert_eq!(
            nom::Err::Error(error::Error { input: input.as_slice(), code: error::ErrorKind::Fail }),
            UpVersionHeader::parse(input).unwrap_err()
        );
    }

    #[test]
    fn parse_header_v1_incomplete_salt() {
        let input = &[0x30];
        let err = UpVersionHeader::parse(input).unwrap_err();
        match err {
            nom::Err::Error(e) => assert_eq!(e.code, error::ErrorKind::Eof),
            _ => panic!("Unexpected error: {:?}", err),
        }
    }

    #[test]
    fn parse_header_v1_nonzero_reserved() {
        let input = &[0x21];
        assert_eq!(
            nom::Err::Error(error::Error { input: input.as_slice(), code: error::ErrorKind::Fail }),
            UpVersionHeader::parse(input).unwrap_err()
        );
    }

    #[test]
    fn parse_header_bad_version() {
        let input = &[0x80];
        assert_eq!(
            nom::Err::Error(error::Error { input: input.as_slice(), code: error::ErrorKind::Fail }),
            UpVersionHeader::parse(input).unwrap_err()
        );
    }

    #[test]
    fn parse_header_v1_simple() {
        let (_, header) = UpVersionHeader::parse(&[0x20]).unwrap();
        assert_eq!(UpVersionHeader::V1(V1AdvHeader::new(0x20, None, None)), header);
    }

    #[test]
    fn parse_header_v1_with_hash() {
        let mut input = std::vec![0x28]; // 0x20 | 0x08
        let hash = [0x01, 0x02, 0x03];
        input.extend_from_slice(&hash);
        let (rem, header) = UpVersionHeader::parse(&input).unwrap();
        assert_eq!(rem.len(), 0);
        assert_eq!(UpVersionHeader::V1(V1AdvHeader::new(0x28, Some(hash), None)), header);
    }

    #[test]
    fn parse_header_v1_with_salt() {
        let mut input = std::vec![0x30]; // 0x20 | 0x10
        let salt = [0xAA; V1_HEADER_SALT_LEN];
        input.extend_from_slice(&salt);
        let (rem, header) = UpVersionHeader::parse(&input).unwrap();
        assert_eq!(rem.len(), 0);
        assert_eq!(UpVersionHeader::V1(V1AdvHeader::new(0x30, None, Some(salt))), header);
    }

    #[test]
    fn parse_header_v1_with_hash_and_salt() {
        let mut input = std::vec![0x38]; // 0x20 | 0x08 | 0x10
        let hash = [0x01, 0x02, 0x03];
        let salt = [0xBB; V1_HEADER_SALT_LEN];
        input.extend_from_slice(&hash);
        input.extend_from_slice(&salt);
        let (rem, header) = UpVersionHeader::parse(&input).unwrap();
        assert_eq!(rem.len(), 0);
        assert_eq!(UpVersionHeader::V1(V1AdvHeader::new(0x38, Some(hash), Some(salt))), header);
    }
}
