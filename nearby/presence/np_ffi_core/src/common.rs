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
//! Common externally-accessible FFI constructs which are needed
//! in order to define the interfaces in this crate's various modules.

use crate::utils::FfiEnum;
use crate::CryptoProviderImpl;
use array_view::ArrayView;
use crypto_provider::{CryptoProvider, CryptoRng};
use handle_map::HandleNotPresentError;
use lock_adapter::stdlib::{RwLock, RwLockWriteGuard};
use lock_adapter::RwLock as _;
use np_adv::legacy::data_elements::psm::PsmDataElement;
use np_adv_dynamic::legacy::ToBoxedSerializeDataElement;

const MAX_HANDLES: u32 = u32::MAX - 1;

/// Configuration for top-level constants to be used
/// by the rest of the FFI which are independent of
/// the programming language which we ultimately
/// interface with at a higher level.
#[repr(C)]
pub struct CommonConfig {
    /// The number of shards to employ in all handle-maps,
    /// or zero if we want to use the default.
    ///
    /// The default number of shards will depend on whether
    /// this crate was compiled with the `std` feature or not:
    /// - If compiled with the `std` feature, the default number
    ///   of shards will be set to
    ///   `min(16, std::thread::available_parallelism().unwrap())`,
    ///   assuming that that call completes successfully.
    /// - In all other cases, 16 shards will be used by default.
    num_shards: u8,
}

impl Default for CommonConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl CommonConfig {
    pub(crate) const fn new() -> Self {
        Self { num_shards: 0 }
    }
    #[cfg(feature = "std")]
    pub(crate) fn num_shards(&self) -> u8 {
        if self.num_shards == 0 {
            match std::thread::available_parallelism() {
                Ok(parallelism) => 16u8.min(parallelism),
                Err(_) => 16u8,
            }
        } else {
            self.num_shards
        }
    }
    #[cfg(not(feature = "std"))]
    pub(crate) fn num_shards(&self) -> u8 {
        if self.num_shards == 0 {
            16u8
        } else {
            self.num_shards
        }
    }
    pub(crate) fn set_num_shards(&mut self, num_shards: u8) {
        self.num_shards = num_shards
    }
}

pub(crate) fn default_handle_map_dimensions() -> handle_map::HandleMapDimensions {
    handle_map::HandleMapDimensions {
        num_shards: global_num_shards(),
        max_active_handles: MAX_HANDLES,
    }
}

static COMMON_CONFIG: RwLock<CommonConfig> = RwLock::new(CommonConfig::new());

fn global_num_shards() -> u8 {
    COMMON_CONFIG.read().num_shards()
}

/// Sets an override to the number of shards to employ in the NP FFI's
/// internal handle-maps, which places an upper bound on the number
/// of writing threads which may make progress at any one time
/// when concurrently accessing handles of the same type.
///
/// By default, this value will be set to 16, or in `std` environments,
/// the minimum of 16 and the number of available hardware threads.
/// A shard value override of zero will be interpreted the same
/// as this default.
///
/// Setting this value will have no effect if the handle-maps for the
/// API have already begun being used by the client code, and any
/// values set will take effect upon the first usage of _any_ non-`global_config_set`
/// API call.
pub fn global_config_set_num_shards(num_shards: u8) {
    let mut config = COMMON_CONFIG.write();
    config.set_num_shards(num_shards);
}

/// Holds the count of handles currently allocated for each handle type
#[repr(C)]
pub struct CurrentHandleAllocations {
    cred_book: u32,
    cred_slab: u32,
    decrypted_metadata: u32,
    v0_payload: u32,
    legible_v1_sections: u32,
    v0_advertisement_builder: u32,
    v1_advertisement_builder: u32,
}

/// Returns the count of currently allocated handle types being held by the rust layer. Useful
/// for debugging, logging, and testing.
pub fn global_config_get_current_allocation_count() -> CurrentHandleAllocations {
    CurrentHandleAllocations {
        cred_book: crate::credentials::CredentialBook::get_current_allocation_count(),
        cred_slab: crate::credentials::CredentialSlab::get_current_allocation_count(),
        decrypted_metadata: crate::deserialize::DecryptedMetadata::get_current_allocation_count(),
        v0_payload: crate::deserialize::v0::V0Payload::get_current_allocation_count(),
        legible_v1_sections:
            crate::deserialize::v1::LegibleV1Sections::get_current_allocation_count(),
        v0_advertisement_builder:
            crate::serialize::v0::V0AdvertisementBuilder::get_current_allocation_count(),
        v1_advertisement_builder:
            crate::serialize::v1::V1AdvertisementBuilder::get_current_allocation_count(),
    }
}

/// A result-type enum which tells the caller whether/not a deallocation
/// succeeded or failed due to the requested handle not being present.
#[repr(C)]
pub enum DeallocateResult {
    /// The requested handle to deallocate was not present in the map
    NotPresent = 1,
    /// The object behind the handle was successfully deallocated
    Success = 2,
}

impl From<Result<(), HandleNotPresentError>> for DeallocateResult {
    fn from(result: Result<(), HandleNotPresentError>) -> Self {
        match result {
            Ok(_) => DeallocateResult::Success,
            Err(_) => DeallocateResult::NotPresent,
        }
    }
}

/// Represents the raw contents of the service payload data
/// under the Nearby Presence service UUID
#[repr(C)]
pub struct RawAdvertisementPayload {
    bytes: ByteBuffer<255>,
}

impl RawAdvertisementPayload {
    /// Yields a slice of the bytes in this raw advertisement payload.
    #[allow(clippy::unwrap_used)]
    pub fn as_slice(&self) -> &[u8] {
        // The unwrapping here will never trigger a panic,
        // because the byte-buffer is 255 bytes, the byte-length
        // of which is the maximum value storable in a u8.
        self.bytes.as_slice().unwrap()
    }
}

/// A byte-string with a maximum size of N,
/// where only the first `len` bytes are considered
/// to contain the actual payload. N is only
/// permitted to be between 0 and 255.
#[derive(Clone)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, arbitrary::Arbitrary))]
#[repr(C)]
// TODO: Once generic const exprs are stabilized,
// we could instead make N into a compile-time u8.
pub struct ByteBuffer<const N: usize> {
    /// The number of bytes in the buffer.
    pub len: u8,
    /// The bytes in the buffer.
    pub bytes: [u8; N],
}

impl<const N: usize> Default for ByteBuffer<N> {
    fn default() -> Self {
        Self { len: 0, bytes: [0u8; N] }
    }
}

#[cfg(any(test, feature = "testing"))]
impl<const N: usize> PartialEq for ByteBuffer<N> {
    fn eq(&self, other: &Self) -> bool {
        if self.len != other.len {
            return false;
        }
        // Lengths equal, but not necessarily validated.
        // If the lengths are greater than N, both
        // are invalid structures, but they could still be equal.
        let len = self.len as usize;
        if len > N {
            self.bytes == other.bytes
        } else {
            self.bytes[..len] == other.bytes[..len]
        }
    }
}

#[cfg(any(test, feature = "testing"))]
impl<const N: usize> Eq for ByteBuffer<N> {}

impl<const N: usize> ByteBuffer<N> {
    /// Constructs a byte-buffer from a Rust-side-derived
    /// ArrayView, which is assumed to be trusted to be
    /// properly initialized, and with a size-bound
    /// under 255 bytes.
    pub fn from_array_view(array_view: ArrayView<u8, N>) -> Self {
        let (len, bytes) = array_view.into_raw_parts();
        let len = len as u8;
        Self { len, bytes }
    }
    /// Constructs a byte-buffer from a Rust-side-derived
    /// tinyvec::ArrayVec, which is assumed to be trusted
    /// to be properly initialized, and with a size-bound
    /// under 255 bytes.
    pub fn from_array_vec(array_vec: tinyvec::ArrayVec<[u8; N]>) -> Self {
        let len = array_vec.len() as u8;
        let bytes = array_vec.into_inner();
        Self { len, bytes }
    }
    /// Yields a slice of the first `self.len` bytes of `self.bytes`.
    pub fn as_slice(&self) -> Result<&[u8], InvalidStackDataStructure> {
        if self.len as usize <= N {
            Ok(&self.bytes[..(self.len as usize)])
        } else {
            Err(InvalidStackDataStructure)
        }
    }
}

impl<const N: usize> TryFrom<ByteBuffer<N>> for tinyvec::ArrayVec<[u8; N]> {
    type Error = InvalidStackDataStructure;
    fn try_from(buffer: ByteBuffer<N>) -> Result<Self, Self::Error> {
        if buffer.len as usize <= N {
            Ok(tinyvec::ArrayVec::from_array_len(buffer.bytes, buffer.len as usize))
        } else {
            Err(InvalidStackDataStructure)
        }
    }
}

/// A FFI safe wrapper of a fixed size array
#[derive(Clone)]
#[repr(C)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq, Eq, arbitrary::Arbitrary))]
pub struct FixedSizeArray<const N: usize>([u8; N]);

impl<const N: usize> FixedSizeArray<N> {
    /// Yields a slice of the bytes
    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl<const N: usize> From<FixedSizeArray<N>> for [u8; N] {
    fn from(arr: FixedSizeArray<N>) -> Self {
        arr.0
    }
}

impl<const N: usize> From<[u8; N]> for FixedSizeArray<N> {
    fn from(arr: [u8; N]) -> Self {
        Self(arr)
    }
}

/// Discriminant for [`OptionalFixedSizeArray`].
#[derive(Copy, Clone)]
#[repr(u8)]
pub enum OptionalFixedSizeArrayKind {
    /// The optional fixed-size array isn't specified.
    NotSpecified = 0,
    /// The optional fixed-size array is present.
    Present = 1,
}

/// A FFI safe wrapper of an optional fixed-size byte array.
#[derive(Clone, Default)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq, Eq, arbitrary::Arbitrary))]
#[repr(C)]
pub enum OptionalFixedSizeArray<const N: usize> {
    /// The optional fixed-size array isn't specified.
    #[default]
    NotSpecified,
    /// The optional fixed-size array is present.
    Present(FixedSizeArray<N>),
}

impl<const N: usize> FfiEnum for OptionalFixedSizeArray<N> {
    type Kind = OptionalFixedSizeArrayKind;
    fn kind(&self) -> Self::Kind {
        match self {
            Self::NotSpecified => OptionalFixedSizeArrayKind::NotSpecified,
            Self::Present(_) => OptionalFixedSizeArrayKind::Present,
        }
    }
}

impl<const N: usize> OptionalFixedSizeArray<N> {
    /// Attempts to cast `self` to the `Present` variant,
    /// returning `None` in the case where the passed value is of a different enum variant.
    pub fn into_present(self) -> Option<FixedSizeArray<N>> {
        match self {
            Self::NotSpecified => None,
            Self::Present(x) => Some(x),
        }
    }
    /// Returns true iff this `OptionalFixedSizeArray` has contents.
    pub fn is_present(&self) -> bool {
        match self {
            Self::NotSpecified => false,
            Self::Present(_) => true,
        }
    }
    /// Alias for the `From` method which can save type annotations.
    pub fn into_rust(self) -> Option<[u8; N]> {
        self.into()
    }
}

impl<const N: usize> From<Option<[u8; N]>> for OptionalFixedSizeArray<N> {
    fn from(arr: Option<[u8; N]>) -> Self {
        match arr {
            None => Self::NotSpecified,
            Some(x) => Self::Present(x.into()),
        }
    }
}

impl<const N: usize> From<OptionalFixedSizeArray<N>> for Option<[u8; N]> {
    fn from(arr: OptionalFixedSizeArray<N>) -> Self {
        match arr {
            OptionalFixedSizeArray::NotSpecified => None,
            OptionalFixedSizeArray::Present(x) => Some(x.into()),
        }
    }
}

/// Discriminant for [`OptionalByteBuffer`].
#[derive(Copy, Clone)]
#[repr(u8)]
pub enum OptionalByteBufferKind {
    /// The optional byte buffer isn't specified.
    NotSpecified = 0,
    /// The optional byte buffer is present.
    Present = 1,
}

/// A FFI safe wrapper of an optional byte buffer.
#[derive(Clone, Default)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq, Eq, arbitrary::Arbitrary))]
#[repr(C)]
pub enum OptionalByteBuffer<const N: usize> {
    /// The optional byte buffer isn't specified.
    #[default]
    NotSpecified,
    /// The optional byte buffer is present.
    Present(ByteBuffer<N>),
}

impl<const N: usize> FfiEnum for OptionalByteBuffer<N> {
    type Kind = OptionalByteBufferKind;
    fn kind(&self) -> Self::Kind {
        match self {
            Self::NotSpecified => OptionalByteBufferKind::NotSpecified,
            Self::Present(_) => OptionalByteBufferKind::Present,
        }
    }
}

impl<const N: usize> OptionalByteBuffer<N> {
    /// Attempts to cast `self` to the `Present` variant,
    /// returning `None` in the case where the passed value is of a different enum variant.
    pub fn into_present(self) -> Option<ByteBuffer<N>> {
        match self {
            Self::NotSpecified => None,
            Self::Present(x) => Some(x),
        }
    }
    /// Returns true iff this `OptionalByteBuffer` has contents.
    pub fn is_present(&self) -> bool {
        match self {
            Self::NotSpecified => false,
            Self::Present(_) => true,
        }
    }
}

pub(crate) type CryptoRngImpl = <CryptoProviderImpl as CryptoProvider>::CryptoRng;

pub(crate) struct LazyInitCryptoRng {
    maybe_rng: Option<CryptoRngImpl>,
}

impl LazyInitCryptoRng {
    const fn new() -> Self {
        Self { maybe_rng: None }
    }
    pub(crate) fn get_rng(&mut self) -> &mut CryptoRngImpl {
        self.maybe_rng.get_or_insert_with(CryptoRngImpl::new)
    }
}

/// Shared, lazily-initialized cryptographically-secure
/// RNG for all operations in the FFI core.
static CRYPTO_RNG: RwLock<LazyInitCryptoRng> = RwLock::new(LazyInitCryptoRng::new());

/// Gets a write guard to the (lazily-init) library-global crypto rng.
pub(crate) fn get_global_crypto_rng() -> RwLockWriteGuard<'static, LazyInitCryptoRng> {
    CRYPTO_RNG.write()
}

/// Error returned if the bit representation of a supposedly-Rust-constructed
/// -and-validated type actually doesn't correspond to the format of the
/// data structure expected on the Rust side of the boundary, and performing
/// further operations on the structure would yield unintended behavior.
/// If this kind of error is being raised, the foreign lang code must
/// be messing with stack-allocated data structures for this library
/// in an entirely unexpected way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidStackDataStructure;

/// Discriminant for `BuildTxPowerResult`.
#[repr(u8)]
#[derive(Clone, Copy)]
pub enum BuildTxPowerResultKind {
    /// The transmission power was outside the
    /// allowed -100dBm to 20dBm range.
    OutOfRange = 0,
    /// The transmission power was in range,
    /// and so a `TxPower` struct was constructed.
    Success = 1,
}

/// Result type for attempting to construct a
/// Tx Power from a signed byte.
#[repr(C)]
#[allow(missing_docs)]
pub enum BuildTxPowerResult {
    OutOfRange,
    Success(TxPower),
}

impl FfiEnum for BuildTxPowerResult {
    type Kind = BuildTxPowerResultKind;
    fn kind(&self) -> Self::Kind {
        match self {
            Self::OutOfRange => BuildTxPowerResultKind::OutOfRange,
            Self::Success(_) => BuildTxPowerResultKind::Success,
        }
    }
}

impl BuildTxPowerResult {
    declare_enum_cast! {into_success, Success, TxPower}
}

/// Representation of a transmission power,
/// as used for the Tx Power DE in V0 and V1.
#[derive(Clone, Copy)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq, Eq, arbitrary::Arbitrary))]
#[repr(C)]
pub struct TxPower {
    pub(crate) tx_power: i8,
}

impl TxPower {
    /// Attempts to construct a new TxPower from the given signed-byte value.
    pub fn build_from_signed_byte(tx_power: i8) -> BuildTxPowerResult {
        match np_adv::shared_data::TxPower::try_from(tx_power) {
            Ok(_) => BuildTxPowerResult::Success(Self { tx_power }),
            Err(_) => BuildTxPowerResult::OutOfRange,
        }
    }
    /// Yields this Tx Power value as an i8.
    pub fn as_i8(&self) -> i8 {
        self.tx_power
    }
}

#[cfg(any(test, feature = "testing"))]
impl TxPower {
    /// Constructs a new transmission power in an unchecked fashion
    /// from an `i8`.
    pub fn from_i8_unchecked(tx_power: i8) -> Self {
        Self { tx_power }
    }
}

impl From<np_adv::shared_data::TxPower> for TxPower {
    fn from(tx_power: np_adv::shared_data::TxPower) -> Self {
        Self { tx_power: tx_power.as_i8() }
    }
}

impl TryFrom<TxPower> for np_adv::shared_data::TxPower {
    type Error = InvalidStackDataStructure;
    fn try_from(value: TxPower) -> Result<Self, InvalidStackDataStructure> {
        np_adv::shared_data::TxPower::try_from(value.as_i8()).map_err(|_| InvalidStackDataStructure)
    }
}

/// Discriminant for `BuildPsmResult`.
#[repr(u8)]
#[derive(Clone, Copy)]
pub enum BuildPsmResultKind {
    /// The PSM should be within two bytes.
    OutOfRange = 0,
    /// The PSM was in range,
    /// and so a `Psm` struct was constructed.
    Success = 1,
}

/// Result type for attempting to construct a
/// PSM from two bytes.
#[repr(C)]
#[allow(missing_docs)]
pub enum BuildPsmResult {
    OutOfRange,
    Success(Psm),
}

impl FfiEnum for BuildPsmResult {
    type Kind = BuildPsmResultKind;
    fn kind(&self) -> Self::Kind {
        match self {
            Self::OutOfRange => BuildPsmResultKind::OutOfRange,
            Self::Success(_) => BuildPsmResultKind::Success,
        }
    }
}

impl BuildPsmResult {
    declare_enum_cast! {into_success, Success, Psm}
}

/// Representation of a PSM for L2CAP,
#[derive(Clone, Copy)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq, Eq, arbitrary::Arbitrary))]
#[repr(C)]
pub struct Psm {
    pub(crate) value: u16,
}

impl Psm {
    /// Constructs a new PSM from the given unsigned two-byte value.
    pub fn build_from_unsigned_bytes(psm: u16) -> BuildPsmResult {
        BuildPsmResult::Success(Self { value: psm })
    }

    /// Returns the PSM value.
    pub fn as_u16(&self) -> u16 {
        self.value
    }
}

impl TryFrom<Psm> for ToBoxedSerializeDataElement {
    type Error = InvalidStackDataStructure;

    fn try_from(psm: Psm) -> Result<Self, Self::Error> {
        Ok(ToBoxedSerializeDataElement::Plaintext(Box::new(PsmDataElement { value: psm.value })))
    }
}

/// Represents the contents of a Device Info DE containing the type and name of a device, as well as flag
/// indicating whether the device name is truncated.
///
/// This representation is stable, and so you may directly
/// reference this struct's fields if you wish.
#[derive(Clone)]
#[cfg_attr(any(test, feature = "testing"), derive(Debug, PartialEq, Eq, arbitrary::Arbitrary))]
#[repr(C)]
pub struct DeviceInfo {
    /// The numerical value corresponding to a given
    /// form factor, or "0" for the case where the
    /// broadcasting device doesn't know what kind
    /// of device type it is.
    ///
    /// Consult the Nearby spec for values
    /// corresponding to particular form factors.
    pub device_type: u8,
    /// Boolean flag indicating whether the device name is truncated.
    pub is_name_truncated: bool,
    /// An optional byte array representing the name of the broadcasting
    /// device. The maximum size of this device name array is 9 bytes.
    pub device_name: OptionalByteBuffer<9>,
}

impl TryFrom<DeviceInfo> for ToBoxedSerializeDataElement {
    type Error = InvalidStackDataStructure;

    fn try_from(core_device_info: DeviceInfo) -> Result<Self, Self::Error> {
        let device_type = np_adv::shared_data::DeviceType::from_repr(core_device_info.device_type)
            .ok_or(InvalidStackDataStructure)?;

        let name_slice = match core_device_info.device_name.into_present() {
            Some(buffer) => buffer.as_slice()?.to_vec(),
            None => Vec::new(),
        };

        let device_info = np_adv::shared_data::DeviceInfo::try_from((
            device_type,
            core_device_info.is_name_truncated,
            name_slice.as_slice(),
        ))
        .map_err(|_| InvalidStackDataStructure)?;
        Ok(ToBoxedSerializeDataElement::Plaintext(Box::new(
            np_adv::legacy::data_elements::device_info::DeviceInfoDataElement { device_info },
        )))
    }
}
