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

use common::data_elements::CoreDataElement;
use identity_common::definitions::{DeviceType, VisibilityLevel, VisibilityScope};
use identity_manager::types::{AuthToken, BroadcastSessionId, StableDeviceId};
use std::collections::HashMap;

/// Nearby advertisement format.
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum AdvertisementFormat {
    /// V0 format.
    V0,
    /// V1 format.
    V1,
    /// DCT iOS format (used for Google + Apple interop use cases).
    DctIos,
}

/// Encoding request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodingRequest {
    /// V0 encoding request.
    V0 {
        /// Visibility Level of the request.
        visibility: VisibilityLevel,
        /// Data elements.
        data_elements: Vec<CoreDataElement>,
    },
    /// V1 encoding request.
    V1 {
        /// Map of visibility scope to data elements.
        data_elements: HashMap<VisibilityScope, Vec<CoreDataElement>>,
    },
    /// DCT iOS encoding request.
    DctIos {
        /// Data elements (only public).
        data_elements: Vec<CoreDataElement>,
    },
    /// Control advertisements attempt to induce some action in a nearby device.
    /// E.g. fast init advertisements are used to control when nearby device beging scanning.
    Control {
        /// Is the control advertisement short format.
        short_format: bool,
        /// Data elements for control advertisement.
        data_elements: Vec<CoreDataElement>,
    },
}

/// Advertisement service data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvertisementServiceData {
    /// Service data bytes.
    pub data: Vec<u8>,
}

/// A hint indicating the format of the advertisement content in a scanned result.
/// For BLE, the hint is derived from the service UUID of the advertisement.
/// For other mediums, the hint defaults to `Up`.
#[derive(Debug, Eq, PartialEq, Copy, Clone, Hash)]
pub enum ContentTypeHint {
    /// The content could be in either the V0 (legacy) or V1 format.
    Up,
    /// The advertisement is in a special format for iOS, where the content is
    /// embedded in the BLE device name.
    DctIos,
    /// The advertisement is for the legacy Nearby Connections (NC) protocol.
    ///
    /// This is identified by a dedicated service UUID specific to NC.
    Legacy,
}

/// Decoded result with decoded device information.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DecodedResult {
    /// Service data was not decoded due to an illegal format or a decryption failure.
    NotDecoded,
    /// V0 format decoded.
    V0 {
        /// Device identity of the discovered device.
        device_unique_id: DeviceUniqueId,
        /// Decoded device info for V0 advertisement.
        decoded_info: DecodedDeviceInfo,
    },
    /// DCT format (iOS only)
    DctIos {
        /// Device identity of the discovered device.
        device_unique_id: DeviceUniqueId,
        /// Decoded device info for DCT advertisement.
        decoded_info: DecodedDeviceInfo,
    },
    /// V1 format decoded with multiple sections.
    V1 {
        /// Device identity of the discovered device.
        device_unique_id: DeviceUniqueId,
        /// List of decoded device info for V1 multisection advertisement.
        decoded_info_list: Vec<DecodedDeviceInfo>,
        /// Number of invalid sections that failed to decode in the V1 advertisement.
        invalid_sections_count: usize,
    },
    /// Control advertisement decoded.
    Control {
        /// Control data elements.
        data_elements: Vec<CoreDataElement>,
    },
}

impl DecodedResult {
    /// The advertisement format of the result.
    pub fn advertisement_format(&self) -> Option<AdvertisementFormat> {
        match self {
            DecodedResult::V0 { .. } => Some(AdvertisementFormat::V0),
            DecodedResult::DctIos { .. } => Some(AdvertisementFormat::DctIos),
            DecodedResult::V1 { .. } => Some(AdvertisementFormat::V1),
            _ => None,
        }
    }
}

/// Device information of the decoded incoming advertisement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedDeviceInfo {
    /// Device name of the discovered device.
    pub device_name: String,
    /// Is the device name truncated?
    pub is_name_truncated: bool,
    /// Device image url of the discovered device.
    pub device_image_url: Option<String>,
    /// Device type of the discovered device.
    pub device_type: DeviceType,
    /// Optional authentication token for connection authentication handshake.
    /// Auth token are not available for public visibility devices.
    pub auth_token: Option<AuthToken>,
    /// Visibility scope.
    pub visibility_scope: VisibilityScope,
    /// Associated data elements for the discovered device.
    pub data_elements: Vec<CoreDataElement>,
}

/// Device identifier for the discovered device.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DeviceUniqueId {
    /// Discovered device is non-public with a stable device ID.
    Stable(StableDeviceId),
    /// Discovered device is public with a broadcast session ID.
    Public(BroadcastSessionId),
    /// Discovered device is a public iOS device with a dedupe byte.
    PublicIos {
        /// Device name.
        name: String,
        /// Is device name truncated.
        is_name_truncated: bool,
        /// Dedupe byte.
        dedup_byte: u8,
    },
}
