// Copyright 2025 Google LLC
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

use identity_manager::errors::{CredentialsError, IdentityManagerError};
use thiserror::Error;

/// Encoding with visibility error type for identity manager.
#[derive(Error, Debug)]
pub enum EncodingWithVisibilityError {
    /// Unsupport API request.
    #[error("Unsupported API request")]
    UnsupportedApiRequest,
    /// Invalid V0 encoding request.
    #[error("Invalid V0 encoding request: insufficient advertising space.")]
    InvalidV0EncodingRequest {
        /// Error conversion from up_adv::legacy::serialize::AddDataElementError.
        #[from]
        source: up_adv::legacy::serialize::AddDataElementError,
    },
    /// Invalid V1 encoding request.
    #[error("Invalid V1 encoding request")]
    InvalidV1EncodingRequest {
        /// V1 encoding failure.
        #[from]
        source: InvalidV1EncodingRequest,
    },
    /// Tx power out of range.
    #[error("Tx power out of range.")]
    TxPowerOutOfRange {
        /// Error conversion from up_adv::shared_data::TxPowerOutOfRange.
        #[from]
        source: up_adv::shared_data::TxPowerOutOfRange,
    },
    /// Device info is malformed.
    #[error("Device info DE is malformed.")]
    DeviceInfoMalformed {
        /// Error conversion from up_adv::shared_data::DeviceInfoMalformed.
        error: up_adv::shared_data::DeviceInfoMalformed,
    },
    /// Unencrypted encoding error for V0 advertisements.
    #[error("Unencrypted encoding error for V0 advertisements")]
    LegacyUnencryptedEncodeError,
    /// Ldt encoding error.
    #[error("Ldt encoding error")]
    LdtEncodeError {
        /// Error conversion from up_adv::legacy::serialize::LdtEncodeError.
        error: up_adv::legacy::serialize::LdtEncodeError,
    },
    /// Unencrypted encode error.
    #[error("Unencrypted encoding error")]
    UnencryptedEncodeError {
        /// Error conversion from up_adv::legacy::serialize::UnencryptedEncodeError.
        error: up_adv::legacy::serialize::UnencryptedEncodeError,
    },
    /// Unsupported V0 action.
    #[error("Unsupported V0 action: {0}")]
    UnsupportedV0Action(u32),
    /// V1 Actions error.
    #[error("V1 Actions error")]
    V1ActionsEncodingError {
        /// Actions error.
        #[from]
        source: up_adv::extended::data_elements::ActionsDataElementError,
    },
    /// Credentials error.
    #[error("Credentials error")]
    CredentialsError(#[from] CredentialsError),
    /// Encrypted identity feature is not enabled.
    #[error("Encrypted identity feature is not enabled.")]
    EncryptedIdentityFeatureNotEnabled,
}

/// Invalid V1 encoding request error type for identity manager.
#[derive(Error, Debug)]
pub enum InvalidV1EncodingRequest {
    /// Insufficient V1 advertising space.
    #[error("Insufficient V1 advertising space.")]
    V1AdvError(#[from] up_adv::extended::serialize::AddDataElementError),
    /// Add V1 section error.
    #[error("Section error.")]
    SectionError(#[from] up_adv::extended::serialize::AddSectionError),
    /// Context sync sequence number is out of range.
    #[error("Context sync sequence number is out of range.")]
    ContextSeqError {
        /// Error conversion from up_adv::shared_data::ContextSyncSeqNumOutOfRange.
        error: up_adv::shared_data::ContextSyncSeqNumOutOfRange,
    },
    /// Public sections cannot be merged with non-public sections.
    #[error("Public sections cannot be merged with non-public sections.")]
    InvalidPublicNonPublicMerge,
    /// Invalid connectivity info.
    #[error("Invalid connectivity info: {0}")]
    InvalidConnectivityInfoDataElement(String),
}

impl From<up_adv::extended::serialize::AddDataElementError> for EncodingWithVisibilityError {
    fn from(source: up_adv::extended::serialize::AddDataElementError) -> Self {
        let invalid_v1_error = InvalidV1EncodingRequest::from(source);
        EncodingWithVisibilityError::InvalidV1EncodingRequest { source: invalid_v1_error }
    }
}

impl From<up_adv::extended::serialize::AddSectionError> for EncodingWithVisibilityError {
    fn from(source: up_adv::extended::serialize::AddSectionError) -> Self {
        let invalid_v1_error = InvalidV1EncodingRequest::from(source);
        EncodingWithVisibilityError::InvalidV1EncodingRequest { source: invalid_v1_error }
    }
}

impl From<up_adv::shared_data::ContextSyncSeqNumOutOfRange> for EncodingWithVisibilityError {
    fn from(source: up_adv::shared_data::ContextSyncSeqNumOutOfRange) -> Self {
        let invalid_v1_error = InvalidV1EncodingRequest::ContextSeqError { error: source };
        EncodingWithVisibilityError::InvalidV1EncodingRequest { source: invalid_v1_error }
    }
}

impl From<up_adv::shared_data::DeviceInfoMalformed> for EncodingWithVisibilityError {
    fn from(source: up_adv::shared_data::DeviceInfoMalformed) -> Self {
        EncodingWithVisibilityError::DeviceInfoMalformed { error: source }
    }
}

/// Decoding with visibility error type for identity manager.
#[derive(Error, Debug)]
pub enum DecodingWithVisibilityError {
    /// Invalid visibility scope.
    #[error("Invalid visibility scope")]
    InvalidVisibilityScope,
    /// Advertisement deserialization error.
    #[error("Advertisement deserialization error.")]
    AdvDeserializationError {
        /// Error conversion from up_adv::AdvDeserializationError.
        #[from]
        source: up_adv::AdvDeserializationError,
    },
    /// Actions deserialization error.
    #[error("Actions deserialization error")]
    ActionsDeserializationError {
        /// Conversion for up_adv::extended::data_elements::actions::ActionsDeserializationError.
        error: up_adv::extended::data_elements::actions::ActionsDeserializationError,
    },
    /// V1 Data element deserialization error.
    #[error("V1 Data element deserialization error")]
    V1DataElementDeserializationError(#[from] V1DataElementDeserializationError),
    /// Data element deserialization error.
    #[error("Data element deserialization error")]
    DataElementDeserializeError {
        /// Error conversion from up_adv::legacy::data_elements::DataElementDeserializeError
        error: up_adv::legacy::data_elements::DataElementDeserializeError,
    },
    /// Data element parse error
    #[error("Data element parse error")]
    DataElementParseError {
        /// Error conversion from up_adv::extended::deserialize::data_element::DataElementParseError.
        #[from]
        source: up_adv::extended::deserialize::data_element::DataElementParseError,
    },
    /// TxPower malformed error.
    #[error("Tx power malformed")]
    TxPowerMalformed {
        /// Error conversion from up_adv::extended::data_elements::TxPowerMalformed.
        error: up_adv::extended::data_elements::TxPowerMalformed,
    },
    /// Action ID out of range
    #[error("Action ID out of range")]
    ActionIdOutOfRange {
        /// Error conversion from up_adv::extended::data_elements::actions::ActionIdOutOfRange.
        #[from]
        source: up_adv::extended::data_elements::actions::ActionIdOutOfRange,
    },
    /// Failed to retrieve encrypted metadata for decryption.
    #[error("Metadata retrieval failed: {0}")]
    MetadataRetrievalFailed(String),
    /// The retrieved metadata failed to decrypt.
    #[error("Metadata decryption failed")]
    MetadataDecryptionFailed,
    /// Protobuf decode error.
    #[error("Failed to decode metadata with protobuf library")]
    ProtobufDecodeError {
        /// Error conversion from protobuf::DecodeError.
        #[from]
        source: protobuf::Error,
    },
    /// Credentials error.
    #[error("Credentials error")]
    CredentialsError(#[from] CredentialsError),
    /// Device name String utf8 decoding error.
    #[error("Device name string utf8 decoding error.")]
    DeviceNameDecodingFailure,
    /// Missing broadcast session ID.
    #[error("Missing broadcast session ID.")]
    MissingBroadcastSessionId,
    /// Public V1 section illegally chained with other V1 sections.
    #[error("Invalid public V1 frame chained with other V1 sections.")]
    InvalidPublicV1Frame,
    /// Idenitity manager error.
    #[error("Identity manager error")]
    IdentityManagerError(#[from] IdentityManagerError),
}

/// V1 data element deserialization error.
#[derive(Error, Debug)]
pub enum V1DataElementDeserializationError {
    /// Failed to deserialize connectivity info.
    #[error("Failed to deserialize connectivity info")]
    ConnectivityInfo {
        /// Conversion for up_adv::extended::data_elements::ConnectivityInfoDeserializationError.
        error: up_adv::extended::data_elements::ConnectivityInfoDeserializationError,
    },
    /// Failed to deserialize context sync seq num.
    #[error("Failed to deserialize context sync seq num")]
    ContextSyncSeqNum {
        /// Conversion for up_adv::extended::data_elements::ContextSyncSeqNumMalformed.
        error: up_adv::extended::data_elements::ContextSyncSeqNumMalformed,
    },
    /// Failed to deserialize cast id.
    #[error("Failed to deserialize cast id")]
    CastId {
        /// Conversion for up_adv::extended::data_elements::CastIdDeserializationError.
        error: up_adv::extended::data_elements::CastIdDeserializationError,
    },
    /// Failed to deserialize media dedupe id.
    #[error("Failed to deserialize media dedupe id")]
    MediaDeduplicationId {
        /// Conversion for up_adv::extended::data_elements::MediaDeduplicationIdDeserializationError.
        error: up_adv::extended::data_elements::MediaDeduplicationIdDeserializationError,
    },
    /// Failed to deserialize broadcast session ID.
    #[error("Failed to deserialize broadcast session ID")]
    BroadcastSessionId {
        /// Conversion for up_adv::extended::data_elements::BroadcastSessionIdMalformed.
        error: up_adv::extended::data_elements::BroadcastSessionIdMalformed,
    },
    /// Failed to deserialize capability bitset.
    #[error("Failed to deserialize capability bitset")]
    CapabilityBitset {
        /// Conversion for up_adv::extended::data_elements::CapabilityBitsetDeserializationError.
        error: up_adv::shared_data::CapabilityBitsetDeserializationError,
    },
    /// Device name deserialization error.
    #[error("Device name deserialization error")]
    DeviceNameDeserializationError {
        /// Conversion for up_adv::extended::data_elements::DeviceNameDeserializationError.
        error: up_adv::extended::data_elements::DeviceNameDeserializationError,
    },
    /// Discovery type deserialization error.
    #[error("Discovery type deserialization error")]
    DiscoveryTypeDeserializationError {
        /// Conversion for up_adv::extended::data_elements::DiscoveryTypeDeserializationError.
        error: up_adv::extended::data_elements::DiscoveryTypeMalformed,
    },
    /// Device type deserialization error.
    #[error("Device type deserialization error")]
    DeviceTypeDeserializationError {
        /// Conversion for up_adv::extended::data_elements::DeviceTypeDeserializationError.
        error: up_adv::extended::data_elements::DeviceTypeDeserializationError,
    },
}

impl From<up_adv::extended::data_elements::TxPowerMalformed> for DecodingWithVisibilityError {
    fn from(source: up_adv::extended::data_elements::TxPowerMalformed) -> Self {
        DecodingWithVisibilityError::TxPowerMalformed { error: source }
    }
}

impl From<up_adv::extended::data_elements::actions::ActionsDeserializationError>
    for DecodingWithVisibilityError
{
    fn from(source: up_adv::extended::data_elements::actions::ActionsDeserializationError) -> Self {
        DecodingWithVisibilityError::ActionsDeserializationError { error: source }
    }
}

impl From<up_adv::extended::data_elements::ConnectivityInfoDeserializationError>
    for DecodingWithVisibilityError
{
    fn from(source: up_adv::extended::data_elements::ConnectivityInfoDeserializationError) -> Self {
        let v1_error = V1DataElementDeserializationError::ConnectivityInfo { error: source };
        DecodingWithVisibilityError::from(v1_error)
    }
}

impl From<up_adv::extended::data_elements::ContextSyncSeqNumMalformed>
    for DecodingWithVisibilityError
{
    fn from(source: up_adv::extended::data_elements::ContextSyncSeqNumMalformed) -> Self {
        let v1_error = V1DataElementDeserializationError::ContextSyncSeqNum { error: source };
        DecodingWithVisibilityError::from(v1_error)
    }
}

impl From<up_adv::extended::data_elements::CastIdDeserializationError>
    for DecodingWithVisibilityError
{
    fn from(source: up_adv::extended::data_elements::CastIdDeserializationError) -> Self {
        let v1_error = V1DataElementDeserializationError::CastId { error: source };
        DecodingWithVisibilityError::from(v1_error)
    }
}

impl From<up_adv::extended::data_elements::MediaDeduplicationIdDeserializationError>
    for DecodingWithVisibilityError
{
    fn from(
        source: up_adv::extended::data_elements::MediaDeduplicationIdDeserializationError,
    ) -> Self {
        let v1_error = V1DataElementDeserializationError::MediaDeduplicationId { error: source };
        DecodingWithVisibilityError::from(v1_error)
    }
}

impl From<up_adv::extended::data_elements::BroadcastSessionIdMalformed>
    for DecodingWithVisibilityError
{
    fn from(source: up_adv::extended::data_elements::BroadcastSessionIdMalformed) -> Self {
        let v1_error = V1DataElementDeserializationError::BroadcastSessionId { error: source };
        DecodingWithVisibilityError::from(v1_error)
    }
}

impl From<up_adv::shared_data::CapabilityBitsetDeserializationError>
    for DecodingWithVisibilityError
{
    fn from(source: up_adv::shared_data::CapabilityBitsetDeserializationError) -> Self {
        let v1_error = V1DataElementDeserializationError::CapabilityBitset { error: source };
        DecodingWithVisibilityError::from(v1_error)
    }
}

impl From<up_adv::extended::data_elements::DeviceNameDeserializationError>
    for DecodingWithVisibilityError
{
    fn from(source: up_adv::extended::data_elements::DeviceNameDeserializationError) -> Self {
        let v1_error =
            V1DataElementDeserializationError::DeviceNameDeserializationError { error: source };
        DecodingWithVisibilityError::from(v1_error)
    }
}

impl From<up_adv::extended::data_elements::DiscoveryTypeMalformed> for DecodingWithVisibilityError {
    fn from(source: up_adv::extended::data_elements::DiscoveryTypeMalformed) -> Self {
        let v1_error =
            V1DataElementDeserializationError::DiscoveryTypeDeserializationError { error: source };
        DecodingWithVisibilityError::from(v1_error)
    }
}

impl From<up_adv::extended::data_elements::DeviceTypeDeserializationError>
    for DecodingWithVisibilityError
{
    fn from(source: up_adv::extended::data_elements::DeviceTypeDeserializationError) -> Self {
        let v1_error =
            V1DataElementDeserializationError::DeviceTypeDeserializationError { error: source };
        DecodingWithVisibilityError::from(v1_error)
    }
}
