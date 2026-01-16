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

//! Common externally-acessible V0 constructs for both of the
//! serialization+deserialization flows.

use crate::common::{
    ByteBuffer, DeviceInfo, InvalidStackDataStructure, OptionalByteBuffer, Psm, TxPower,
};
use crate::serialize::v0::AdvertisementBuilderKind;
use crate::utils::FfiEnum;
use np_adv::{
    legacy::data_elements::actions::ActionsDataElement,
    legacy::{data_elements as np_adv_de, Ciphertext, PacketFlavorEnum, Plaintext},
};

/// Discriminant for `V0DataElement`.
#[repr(u8)]
pub enum V0DataElementKind {
    /// A transmission Power (Tx Power) data-element.
    /// The associated payload may be obtained via
    /// `V0DataElement#into_tx_power`.
    TxPower = 1,
    /// The Actions data-element.
    /// The associated payload may be obtained via
    /// `V0DataElement#into_actions`.
    Actions = 2,
    /// The PSM data-element.
    /// The associated payload may be obtained via
    /// `V0DataElement#into_psm`.
    Psm = 3,
    /// The DeviceInfo data-element.
    /// The associated payload may be obtained via
    /// 'V0DataElement#into_device_info`.
    DeviceInfo = 4,
}

/// Representation of a V0 data element.
#[repr(C)]
#[allow(missing_docs)]
#[derive(Clone)]
pub enum V0DataElement {
    TxPower(TxPower),
    Actions(V0Actions),
    Psm(Psm),
    DeviceInfo(DeviceInfo),
}

impl TryFrom<V0DataElement> for np_adv_dynamic::legacy::ToBoxedSerializeDataElement {
    type Error = InvalidStackDataStructure;
    fn try_from(de: V0DataElement) -> Result<Self, InvalidStackDataStructure> {
        match de {
            V0DataElement::TxPower(x) => x.try_into(),
            V0DataElement::Actions(x) => x.try_into(),
            V0DataElement::Psm(x) => x.try_into(),
            V0DataElement::DeviceInfo(x) => x.try_into(),
        }
    }
}

impl<F: np_adv::legacy::PacketFlavor> From<np_adv::legacy::deserialize::DeserializedDataElement<F>>
    for V0DataElement
{
    fn from(de: np_adv::legacy::deserialize::DeserializedDataElement<F>) -> Self {
        use np_adv::legacy::deserialize::DeserializedDataElement;
        match de {
            DeserializedDataElement::Actions(x) => V0DataElement::Actions(x.into()),
            DeserializedDataElement::TxPower(x) => V0DataElement::TxPower(x.into()),
            DeserializedDataElement::Psm(x) => V0DataElement::Psm(x.into()),
            DeserializedDataElement::DeviceInfo(x) => V0DataElement::DeviceInfo(x.into()),
        }
    }
}

impl FfiEnum for V0DataElement {
    type Kind = V0DataElementKind;
    fn kind(&self) -> Self::Kind {
        match self {
            V0DataElement::Actions(_) => V0DataElementKind::Actions,
            V0DataElement::TxPower(_) => V0DataElementKind::TxPower,
            V0DataElement::Psm(_) => V0DataElementKind::Psm,
            V0DataElement::DeviceInfo(_) => V0DataElementKind::DeviceInfo,
        }
    }
}

impl V0DataElement {
    declare_enum_cast! {into_tx_power, TxPower, TxPower}
    declare_enum_cast! {into_actions, Actions, V0Actions}
    declare_enum_cast! {into_psm, Psm, Psm}
    declare_enum_cast! {into_device_info, DeviceInfo, DeviceInfo}
}

impl From<np_adv_de::tx_power::TxPowerDataElement> for TxPower {
    fn from(de: np_adv_de::tx_power::TxPowerDataElement) -> Self {
        Self { tx_power: de.tx_power_value() }
    }
}

impl From<np_adv_de::psm::PsmDataElement> for Psm {
    fn from(de: np_adv_de::psm::PsmDataElement) -> Self {
        Self { value: de.value }
    }
}

impl From<np_adv_de::device_info::DeviceInfoDataElement> for DeviceInfo {
    fn from(de: np_adv_de::device_info::DeviceInfoDataElement) -> Self {
        let name_slice = de.device_info.device_name();
        let mut name_bytes = [0; 9];
        let len = name_slice.len().min(name_bytes.len());
        #[allow(clippy::expect_used)]
        name_bytes
            .get_mut(..len)
            .expect("slice length is within bounds")
            .copy_from_slice(name_slice.get(..len).expect("slice length is within bounds"));
        let device_name =
            OptionalByteBuffer::Present(ByteBuffer { len: len as u8, bytes: name_bytes });
        Self {
            device_type: de.device_info.device_type() as u8,
            is_name_truncated: de.device_info.name_truncated(),
            device_name,
        }
    }
}

/// Representation of the Actions DE in V0.
#[derive(Clone, Copy)]
#[repr(C)]
pub enum V0Actions {
    /// A set of action bits which were present in a plaintext identity advertisement
    Plaintext(V0ActionBits),
    /// A set of action bits which were present in a encrypted identity advertisement
    Encrypted(V0ActionBits),
}

impl TryFrom<V0Actions> for np_adv_dynamic::legacy::ToBoxedSerializeDataElement {
    type Error = InvalidStackDataStructure;
    fn try_from(value: V0Actions) -> Result<Self, InvalidStackDataStructure> {
        let boxed_action_bits = np_adv_dynamic::legacy::BoxedActionBits::try_from(value)?;
        Ok(boxed_action_bits.into())
    }
}

impl<F: np_adv::legacy::PacketFlavor> From<ActionsDataElement<F>> for V0Actions {
    fn from(value: ActionsDataElement<F>) -> Self {
        match F::ENUM_VARIANT {
            PacketFlavorEnum::Plaintext => {
                Self::Plaintext(V0ActionBits { bitfield: value.action.as_u32() })
            }
            PacketFlavorEnum::Ciphertext => {
                Self::Encrypted(V0ActionBits { bitfield: value.action.as_u32() })
            }
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
/// The bitfield data of a V0Actions data element
pub struct V0ActionBits {
    bitfield: u32,
}

impl From<u32> for V0ActionBits {
    fn from(bitfield: u32) -> Self {
        // No need to validate here. Validation of this struct is done at all places where it is
        // taken as a parameter. See [`InvalidStackDataStructure`]. Since this struct is
        // `#[repr(C)]` if rules were enforced here, they could be broken by a foreign language
        // user.
        Self { bitfield }
    }
}

#[derive(Clone, Copy)]
#[allow(missing_docs)]
#[repr(C)]
/// Action data element
pub struct ActionType(u8);
const MAX_V0_ACTION_ID: u8 = 31;

#[derive(Clone, Copy, Debug)]
/// The given int is out of range for conversion.
pub struct TryFromIntError;

impl From<core::num::TryFromIntError> for TryFromIntError {
    fn from(_: core::num::TryFromIntError) -> Self {
        Self
    }
}

/// Discriminant for `BuildActionTypeResult`.
#[repr(u8)]
#[derive(Clone, Copy)]
pub enum BuildActionTypeResultKind {
    /// The action type was outside the
    /// allowed 0 to 31 range.
    OutOfRange = 0,
    /// The transmission power was in range,
    /// and so a `TxPower` struct was constructed.
    Success = 1,
}

/// Result type for attempting to construct a
/// ActionType from an usigned byte.
#[repr(C)]
#[allow(missing_docs)]
pub enum BuildActionTypeResult {
    OutOfRange,
    Success(ActionType),
}

impl FfiEnum for BuildActionTypeResult {
    type Kind = BuildActionTypeResultKind;
    fn kind(&self) -> Self::Kind {
        match self {
            Self::OutOfRange => BuildActionTypeResultKind::OutOfRange,
            Self::Success(_) => BuildActionTypeResultKind::Success,
        }
    }
}

impl BuildActionTypeResult {
    declare_enum_cast! {into_success, Success, ActionType}
}

impl TryFrom<u8> for ActionType {
    type Error = TryFromIntError;
    fn try_from(n: u8) -> Result<Self, Self::Error> {
        if n > MAX_V0_ACTION_ID {
            Err(TryFromIntError)
        } else {
            Ok(ActionType(n))
        }
    }
}

impl ActionType {
    pub(crate) fn as_boxed_action_element(
        &self,
        value: bool,
    ) -> np_adv_dynamic::legacy::ToBoxedActionElement {
        use np_adv_dynamic::legacy::ToBoxedActionElement;
        ToBoxedActionElement { action: self.0, value }
    }

    /// Constructs a new ActionType from the given unsigned byte value.
    pub fn build_from_unsigned_byte(action_type: u8) -> BuildActionTypeResult {
        match action_type.try_into() {
            Ok(action_type) => BuildActionTypeResult::Success(action_type),
            Err(_) => BuildActionTypeResult::OutOfRange,
        }
    }

    /// Returns the ActionType value.
    pub fn as_u8(&self) -> u8 {
        self.0
    }
}

impl From<ActionType> for np_adv::legacy::data_elements::actions::ActionType {
    fn from(value: ActionType) -> Self {
        np_adv::legacy::data_elements::actions::ActionType(value.0)
    }
}

// ensure bidirectional mapping
impl From<np_adv::legacy::data_elements::actions::ActionType> for ActionType {
    fn from(value: np_adv_de::actions::ActionType) -> Self {
        ActionType(value.0)
    }
}

impl From<np_adv_dynamic::legacy::BoxedActionBits> for V0Actions {
    fn from(bits: np_adv_dynamic::legacy::BoxedActionBits) -> Self {
        use np_adv_dynamic::legacy::BoxedActionBits;
        match bits {
            BoxedActionBits::Plaintext(x) => Self::Plaintext(V0ActionBits { bitfield: x.as_u32() }),
            BoxedActionBits::Ciphertext(x) => {
                Self::Encrypted(V0ActionBits { bitfield: x.as_u32() })
            }
        }
    }
}

impl TryFrom<V0Actions> for np_adv_dynamic::legacy::BoxedActionBits {
    type Error = InvalidStackDataStructure;
    fn try_from(actions: V0Actions) -> Result<Self, InvalidStackDataStructure> {
        match actions {
            V0Actions::Plaintext(action_bits) => {
                let bits = np_adv::legacy::data_elements::actions::ActionBits::<Plaintext>::from(
                    action_bits.bitfield,
                );
                Ok(bits.into())
            }
            V0Actions::Encrypted(action_bits) => {
                let bits = np_adv::legacy::data_elements::actions::ActionBits::<Ciphertext>::from(
                    action_bits.bitfield,
                );
                Ok(bits.into())
            }
        }
    }
}

/// Discriminant for `SetV0ActionResult`.
#[repr(u8)]
pub enum SetV0ActionResultKind {
    /// The attempt to set the action bit failed. The
    /// action bits were yielded back to the caller,
    /// unmodified.
    Error = 0,
    /// The attempt to set the action bit succeeded.
    /// The updated action bits were yielded back to the caller.
    Success = 1,
}

/// The result of attempting to set a particular action
/// bit on some `V0Actions`.
#[repr(C)]
#[allow(missing_docs)]
pub enum SetV0ActionResult {
    Success(V0Actions),
    Error(V0Actions),
}

impl FfiEnum for SetV0ActionResult {
    type Kind = SetV0ActionResultKind;
    fn kind(&self) -> Self::Kind {
        match self {
            Self::Success(_) => SetV0ActionResultKind::Success,
            Self::Error(_) => SetV0ActionResultKind::Error,
        }
    }
}

impl SetV0ActionResult {
    declare_enum_cast! { into_success, Success, V0Actions }
    declare_enum_cast! { into_error, Error, V0Actions }
}

impl V0Actions {
    /// Constructs a new V0 actions DE with no declared boolean
    /// actions and a zeroed context sync sequence number,
    /// where the DE is intended for the given advertisement
    /// kind (plaintext/encrypted).
    pub fn new_zeroed(kind: AdvertisementBuilderKind) -> Self {
        match kind {
            AdvertisementBuilderKind::Public => Self::Plaintext(V0ActionBits { bitfield: 0 }),
            AdvertisementBuilderKind::Encrypted => Self::Encrypted(V0ActionBits { bitfield: 0 }),
        }
    }

    /// Gets the V0 Action bits as represented by a u32 where the last 8 bits are
    /// always 0 since V0 actions can only hold up to 24 bits.
    pub fn as_u32(&self) -> u32 {
        match self {
            V0Actions::Plaintext(bits) => bits.bitfield,
            V0Actions::Encrypted(bits) => bits.bitfield,
        }
    }

    /// Return whether a boolean action type is set in this data element
    #[allow(clippy::expect_used)]
    pub fn has_action(&self, action_type: ActionType) -> Result<bool, InvalidStackDataStructure> {
        let boxed_action_bits = np_adv_dynamic::legacy::BoxedActionBits::try_from(*self)?;
        let action_type = action_type.into();
        Ok(boxed_action_bits.has_action(action_type))
    }

    /// Attempts to set the given action bit to the given boolean value.
    /// This operation may fail if the requested action bit may not be
    /// set for the kind of containing advertisement (public/encrypted)
    /// that this action DE is intended to belong to. In this case,
    /// the original action bits will be yielded back to the caller,
    /// unaltered.
    pub fn set_action(
        self,
        action_type: ActionType,
        value: bool,
    ) -> Result<SetV0ActionResult, InvalidStackDataStructure> {
        let mut boxed_action_bits = np_adv_dynamic::legacy::BoxedActionBits::try_from(self)?;
        let boxed_action_element = action_type.as_boxed_action_element(value);
        match boxed_action_bits.set_action(boxed_action_element) {
            Ok(()) => Ok(SetV0ActionResult::Success(boxed_action_bits.into())),
            Err(_) => Ok(SetV0ActionResult::Error(self)),
        }
    }
}
