// Copyright 2025 Google LLC
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

use std::hash::Hash;

/// Core Nearby data elements for broadcasting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreDataElement {
    //
    // Common data elements for V0 and V1:
    //
    /// Set of service IDs that are supported and active on the broadcasting device.
    ServiceIds(Vec<ServiceId>),
    /// A DE bitmask indicating whether service IDs 0-7 are enabled.
    SupportedServices(u8),
    /// Requirements specified by Fast init.
    Requirements(Vec<DeviceCapability>),
    /// Type of the fast init discovery.
    DiscoveryType(DiscoveryType),
    //
    // V0 only data elements:
    //
    /// L2CAP PSM.
    BleL2capCocPsm(u16),
    //
    // V1 only data elements:
    //
    /// Characteristics identifying the capabilities of an advertising device.
    Capabilities(
        /// Vector of device capabilities.
        Vec<DeviceCapability>,
    ),
    /// Google Cast identifier.
    GoogleCastId([u8; 32]),
    /// Deduplication hint provided by D2DI used for deduping across different discovery media (e.g. Usonia, Cast, Nearby).
    MediaDeduplicationId([u8; 20]),
    /// Context sequence number.
    ContextSequenceNumber(u8),
    /// Tx Power of an advertisement.
    BleTxPower(i8),
    /// Ble connection information.
    BleConnectionInformation {
        /// 6 bytes BLE mac address.
        mac_address: Vec<u8>,
        /// 5 bytes GATT characteristic.
        gatt_characteristic: Vec<u8>,
        /// 2 bytes L2CAP PSM.
        psm: u16,
        /// 2 bytes Device token.
        device_token: u16,
    },
    /// WiFi LAN IPv4 connection information.
    WifiLanIpv4ConnectionInformation {
        /// 4 bytes IPv4 address.
        ip_address: Vec<u8>,
        /// 2 bytes port number.
        port: u16,
        /// Optional 6 bytes BSSID. ([00, 00, 00, 00, 00, 00] if not assigned).
        bssid: Option<Vec<u8>>,
    },
    /// WiFi LAN IPv6 connection information.
    WifiLanIpv6ConnectionInformation {
        /// 16 bytes IPv6 address.
        ip_address: Vec<u8>,
        /// 2 bytes port number.
        port: u16,
        /// Optional 6 bytes BSSID. ([00, 00, 00, 00, 00, 00] if not assigned).
        bssid: Option<Vec<u8>>,
    },
    /// WiFi Aware connection information.
    WifiAwareConnectionInformation {
        /// 4 bytes service specific info.
        service_specific_info: Vec<u8>,
        /// 8 bytes psk password.
        psk_password: Vec<u8>,
        /// 2 bytes port.
        port: u16,
    },
    /// UWB connectivity capability.
    UwbConnectivityCapability {
        /// Supports static sts.
        supports_static_sts: bool,
        /// Supports provisioned sts.
        supports_provisioned_sts: bool,
    },
}

impl CoreDataElement {
    /// If the data element can be included in a short-format advertisement.
    pub fn supports_short_format(&self) -> bool {
        matches!(
            self,
            CoreDataElement::ServiceIds(_)
                | CoreDataElement::Requirements(_)
                | CoreDataElement::DiscoveryType(_)
                | CoreDataElement::BleL2capCocPsm(_)
                | CoreDataElement::Capabilities(_)
                | CoreDataElement::MediaDeduplicationId(_)
                | CoreDataElement::BleTxPower(_)
        )
    }
}

/// Characteristics identifying the capabilities of a device.
pub type DeviceCapability = up_adv::shared_data::DeviceCapability;

/// The type of discovery in Fast Init.
pub type DiscoveryType = up_adv::shared_data::DiscoveryType;

/// Service identifiers used in a broadcast to indicate that a service is enabled on the broadcasting device.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ServiceId(pub u32);
