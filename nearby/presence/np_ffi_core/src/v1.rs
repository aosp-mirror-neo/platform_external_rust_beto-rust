// Copyright 2023 Google LLC
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

//! Common externally-acessible V1 constructs for both of the
//! serialization+deserialization flows.

use crate::common::{
    ByteBuffer, FixedSizeArray, InvalidStackDataStructure, OptionalFixedSizeArray, TxPower,
};
use crate::utils::FfiEnum;

/// Information about the verification scheme used
/// for verifying the integrity of the contents
/// of a decrypted section.
#[derive(Clone, Copy)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq, Eq, arbitrary::Arbitrary))]
#[repr(u8)]
pub enum V1VerificationMode {
    /// Message integrity code verification.
    Mic = 0,
}

impl From<np_adv::extended::deserialize::VerificationMode> for V1VerificationMode {
    fn from(verification_mode: np_adv::extended::deserialize::VerificationMode) -> Self {
        use np_adv::extended::deserialize::VerificationMode;
        match verification_mode {
            VerificationMode::Mic => Self::Mic,
        }
    }
}

/// Representation of the data-element type tag
/// of a V1 data element.
#[derive(Clone, Copy)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq, Eq, arbitrary::Arbitrary))]
#[repr(C)]
pub struct V1DEType {
    /// DE type-code. Valid iff this is not
    /// the forbidden type-code 0xFFFFFFFF.
    code: u32,
}

impl From<np_adv::extended::de_type::DeType> for V1DEType {
    fn from(de_type: np_adv::extended::de_type::DeType) -> Self {
        let code = de_type.as_u32();
        Self { code }
    }
}

impl TryFrom<V1DEType> for np_adv::extended::de_type::DeType {
    type Error = np_adv::extended::de_type::InvalidDeType;
    fn try_from(de_type: V1DEType) -> Result<Self, Self::Error> {
        de_type.code.try_into()
    }
}

impl V1DEType {
    /// Yields this V1 DE type code as a u32.
    pub fn to_u32(&self) -> u32 {
        self.code
    }
}

/// Represents the contents of an arbitrary type-code V1 DE whose payload
/// is stored in a buffer which may contain up to 127 bytes,
/// which is the maximum for any V1 DE.
///
/// This representation is stable, and so you may directly
/// reference this struct's fields if you wish.
#[derive(Clone)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq, Eq, arbitrary::Arbitrary))]
#[repr(C)]
pub struct GenericV1DataElement {
    /// The DE type code of this generic data-element.
    pub de_type: V1DEType,
    /// The raw data-element byte payload, up to
    /// 127 bytes in length.
    pub payload: ByteBuffer<127>,
}

impl GenericV1DataElement {
    /// Gets the DE-type of this generic V1 data element.
    pub fn de_type(&self) -> V1DEType {
        self.de_type
    }
    /// Destructures this `GenericV1DataElement` into just the DE payload byte-buffer.
    pub fn into_payload(self) -> ByteBuffer<127> {
        self.payload
    }
}

/// Discriminant for [`V1DataElement`].
#[derive(Clone, Copy)]
#[repr(u8)]
pub enum V1DataElementKind {
    /// A "generic" V1 data element which may represent any V1
    /// data element, but during the deserialization path,
    /// this will represent a data element whose contents
    /// could not be further deserialized into a stock-recognizable DE.
    Generic = 0,
    /// A Cast Id.
    CastId = 1,
    /// A V1 "Actions" DE.
    Actions = 2,
    /// A transmission power.
    TxPower = 3,
    /// A context sync sequence number.
    ContextSyncSeqNum = 4,
    /// A deduplication hint.
    DedupHint = 5,
    /// A device type.
    DeviceType = 6,
    /// Connectivity information.
    ConnectivityInfo = 7,
}

/// A V1 data element, which may be a "generic" data element
/// or any of a number of Google-defined data elements.
#[derive(Clone)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq, Eq, arbitrary::Arbitrary))]
#[repr(C)]
#[allow(missing_docs)]
pub enum V1DataElement {
    Generic(GenericV1DataElement),
    CastId(CastId),
    Actions(V1ActionsBitmap),
    TxPower(TxPower),
    ContextSyncSeqNum(ContextSyncSeqNum),
    DedupHint(DedupHint),
    DeviceType(DeviceType),
    ConnectivityInfo(ConnectivityInfo),
}

impl FfiEnum for V1DataElement {
    type Kind = V1DataElementKind;
    fn kind(&self) -> Self::Kind {
        match self {
            Self::Generic(_) => V1DataElementKind::Generic,
            Self::CastId(_) => V1DataElementKind::CastId,
            Self::Actions(_) => V1DataElementKind::Actions,
            Self::TxPower(_) => V1DataElementKind::TxPower,
            Self::ContextSyncSeqNum(_) => V1DataElementKind::ContextSyncSeqNum,
            Self::DedupHint(_) => V1DataElementKind::DedupHint,
            Self::DeviceType(_) => V1DataElementKind::DeviceType,
            Self::ConnectivityInfo(_) => V1DataElementKind::ConnectivityInfo,
        }
    }
}

impl V1DataElement {
    declare_enum_cast! {into_generic, Generic, GenericV1DataElement}
    declare_enum_cast! {into_cast_id, CastId, CastId}
    declare_enum_cast! {into_actions, Actions, V1ActionsBitmap}
    declare_enum_cast! {into_tx_power, TxPower, TxPower}
    declare_enum_cast! {into_context_sync_seq_num, ContextSyncSeqNum, ContextSyncSeqNum}
    declare_enum_cast! {into_dedup_hint, DedupHint, DedupHint}
    declare_enum_cast! {into_device_type, DeviceType, DeviceType}
    declare_enum_cast! {into_connectivity_info, ConnectivityInfo, ConnectivityInfo}
}

/// Represents the contents of a V1 Actions DE as a bitmap
/// where the absence/presence of bits represent the absence/presence
/// of action IDs, where the bytes in the bitmap are kept in
/// big-endian ordering and each bit-position within a byte is kept in little-endian.
///
/// This representation is stable, and so you may directly
/// reference this struct's fields if you wish.
#[derive(Clone)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq, Eq, arbitrary::Arbitrary))]
#[repr(C)]
pub struct V1ActionsBitmap {
    /// The underlying byte-array for the bitmap.
    pub bitmap: [u8; 256],
}

/// A Google Cast identifier of 32 bytes.
///
/// This representation is stable, so you may directly
/// reference this struct's fields if you wish.
#[derive(Clone)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq, Eq, arbitrary::Arbitrary))]
#[repr(C)]
pub struct CastId {
    /// The raw bytes of the Cast Id.
    pub bytes: [u8; 32],
}

/// Discriminant for `BuildContextSyncSeqNumResult`.
#[repr(u8)]
#[derive(Clone, Copy)]
pub enum BuildContextSyncSeqNumResultKind {
    /// The context sync sequence number
    /// was outside the allowed 0-15 range.
    OutOfRange = 0,
    /// The context sync sequence number
    /// was in range, and so a `ContextSyncSeqNum`
    /// struct was constructed.
    Success = 1,
}

/// Result type for attempting to construct
/// a context sync sequence number from an unsigned byte.
#[repr(C)]
#[allow(missing_docs)]
pub enum BuildContextSyncSeqNumResult {
    OutOfRange,
    Success(ContextSyncSeqNum),
}

impl FfiEnum for BuildContextSyncSeqNumResult {
    type Kind = BuildContextSyncSeqNumResultKind;
    fn kind(&self) -> Self::Kind {
        match self {
            Self::OutOfRange => BuildContextSyncSeqNumResultKind::OutOfRange,
            Self::Success(_) => BuildContextSyncSeqNumResultKind::Success,
        }
    }
}

impl BuildContextSyncSeqNumResult {
    declare_enum_cast! {into_success, Success, ContextSyncSeqNum}
}

/// A context sync sequence number.
#[derive(Clone, Copy)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq, Eq, arbitrary::Arbitrary))]
#[repr(C)]
pub struct ContextSyncSeqNum {
    /// The sequence number, from 0-15.
    seq_num: u8,
}

#[cfg(any(test, feature = "testing"))]
impl ContextSyncSeqNum {
    /// Constructs a new context-sync sequence number in an unchecked
    /// fashion from the given u8.
    pub fn from_u8_unchecked(seq_num: u8) -> Self {
        Self { seq_num }
    }
}

impl ContextSyncSeqNum {
    /// Attempts to construct a new context sync sequence number
    /// from the given unsigned-byte value.
    pub fn build_from_unsigned_byte(seq_num: u8) -> BuildContextSyncSeqNumResult {
        match np_adv::shared_data::ContextSyncSeqNum::try_from(seq_num) {
            Ok(_) => BuildContextSyncSeqNumResult::Success(Self { seq_num }),
            Err(_) => BuildContextSyncSeqNumResult::OutOfRange,
        }
    }
    /// Yields this context sync sequence number as a u8.
    pub fn as_u8(&self) -> u8 {
        self.seq_num
    }
}

impl From<np_adv::shared_data::ContextSyncSeqNum> for ContextSyncSeqNum {
    fn from(seq_num: np_adv::shared_data::ContextSyncSeqNum) -> Self {
        Self { seq_num: seq_num.as_u8() }
    }
}

impl TryFrom<ContextSyncSeqNum> for np_adv::shared_data::ContextSyncSeqNum {
    type Error = InvalidStackDataStructure;
    fn try_from(seq_num: ContextSyncSeqNum) -> Result<Self, Self::Error> {
        Self::try_from(seq_num.seq_num).map_err(|_| InvalidStackDataStructure)
    }
}

/// A deduplication hint used to identify the same
/// device across different broadcasting media.
///
/// This structure's fields are stable, so you may
/// directly reference them if you wish.
#[repr(C)]
#[derive(Clone)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq, Eq, arbitrary::Arbitrary))]
pub struct DedupHint {
    /// The raw bytes of the deduplication hint.
    pub bytes: [u8; 8],
}

/// A description of what kind of device
/// is broadcasting.
///
/// This structure's fields are stable, so you may
/// directly reference them if you wish.
///
/// Consult the Nearby spec for the numeric
/// values corresponding to particular form factors.
#[repr(C)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq, Eq, arbitrary::Arbitrary))]
#[derive(Clone)]
pub struct DeviceType {
    /// The numerical value corresponding to a given
    /// form factor, or "0" for the case where the
    /// broadcasting device doesn't know what kind
    /// of device type it is.
    ///
    /// Consult the Nearby spec for values
    /// corresponding to particular form factors.
    pub device_type: u8,
}

/// Information for connecting to a broadcasting device.
///
/// You should not depend on the layout details of this
/// structure, since more fields may be added in the future.
#[repr(C)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq, Eq, arbitrary::Arbitrary))]
#[derive(Clone, Default)]
pub struct ConnectivityInfo {
    /// Any BLE-specific connectivity info.
    pub(crate) ble_info: BleConnectivityInfo,
    /// Any Wifi-LAN-specific connectivity info.
    pub(crate) wifi_lan_info: WifiLanConnectivityInfo,
}

impl ConnectivityInfo {
    /// Gets the BLE component of this connectivity info.
    pub fn get_ble_info(&self) -> BleConnectivityInfo {
        self.ble_info.clone()
    }
    /// Gets the Wifi-LAN component of this connectivity info.
    pub fn get_wifi_lan_info(&self) -> WifiLanConnectivityInfo {
        self.wifi_lan_info.clone()
    }
    /// Creates a new connectivity info with the BLE connectivity information
    /// of this instance replaced with the given value.
    pub fn set_ble_info(&self, ble_info: BleConnectivityInfo) -> Self {
        Self { ble_info, ..self.clone() }
    }
    /// Creates a new connectivity info with the Wifi-LAN connectivity information
    /// of this instance replaced with the given value.
    pub fn set_wifi_lan_info(&self, wifi_lan_info: WifiLanConnectivityInfo) -> Self {
        Self { wifi_lan_info, ..self.clone() }
    }
}

/// Information for connecting to a broadcasting device over BLE.
///
/// This structure's fields are stable, so you may
/// directly reference them if you wish.
#[repr(C)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq, Eq, arbitrary::Arbitrary))]
#[derive(Clone, Default)]
pub struct BleConnectivityInfo {
    /// Bluetooth MAC address.
    pub mac_address: OptionalFixedSizeArray<6>,
    /// An identifier for the GATT service to connect to.
    /// This could be a UUID + instance ID (17 bytes),
    /// an attribute handle + instance ID (3 bytes),
    /// or just a UUID (16 bytes) or attribute handle (2 bytes),
    /// or empty (0 bytes) for no GATT service identifier present.
    pub gatt_service_identifier: ByteBuffer<17>,
    /// The PSM (L2CAP) port to connect on.
    pub psm: OptionalFixedSizeArray<2>,
    /// Physical identifier ("device token") for the device.
    pub device_token: OptionalFixedSizeArray<2>,
}

impl BleConnectivityInfo {
    /// Returns true iff the contents of this BLE connectivity info component are non-empty.
    pub fn is_present(&self) -> bool {
        self.mac_address.is_present()
            || self.gatt_service_identifier.as_slice().map(|x| !x.is_empty()).unwrap_or(false)
            || self.psm.is_present()
            || self.device_token.is_present()
    }
}

/// Discriminant for [`OptionalIpAddress`].
#[derive(Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum OptionalIpAddressKind {
    /// No IP address specified.
    NotPresent = 0,
    /// An IPv4 address.
    IPv4 = 1,
    /// An IPv6 address.
    IPv6 = 2,
}

/// An optional IPv4 or IPv6 address, represented by big-endian bytes.
#[repr(C)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq, Eq, arbitrary::Arbitrary))]
#[derive(Clone)]
pub enum OptionalIpAddress {
    /// No IP address specified.
    NotPresent,
    /// An IPv4 address (4 bytes).
    IPv4(FixedSizeArray<4>),
    /// An IPv6 address (16 bytes).
    IPv6(FixedSizeArray<16>),
}

impl Default for OptionalIpAddress {
    fn default() -> Self {
        Self::NotPresent
    }
}

impl FfiEnum for OptionalIpAddress {
    type Kind = OptionalIpAddressKind;
    fn kind(&self) -> Self::Kind {
        match self {
            Self::NotPresent => OptionalIpAddressKind::NotPresent,
            Self::IPv4(_) => OptionalIpAddressKind::IPv4,
            Self::IPv6(_) => OptionalIpAddressKind::IPv6,
        }
    }
}

/// Information for connecting to a broadcasting device over Wifi-LAN.
///
/// This structure's fields are stable, so you may
/// directly reference them if you wish.
#[repr(C)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq, Eq, arbitrary::Arbitrary))]
#[derive(Clone, Default)]
pub struct WifiLanConnectivityInfo {
    /// IP address.
    pub ip: OptionalIpAddress,
    /// Port number, as two bytes in big-endian.
    pub port: OptionalFixedSizeArray<2>,
    /// Six-byte BSSID.
    pub bssid: OptionalFixedSizeArray<6>,
}

impl WifiLanConnectivityInfo {
    /// Returns true iff the contents of this Wifi-LAN connectivity info component are non-empty.
    pub fn is_present(&self) -> bool {
        self.ip.kind() != OptionalIpAddressKind::NotPresent
            || self.port.is_present()
            || self.bssid.is_present()
    }
}
