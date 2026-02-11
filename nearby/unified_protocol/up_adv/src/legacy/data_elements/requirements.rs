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

//! Data element for Requirements.

use crate::legacy::data_elements::de_type::{DeActualLength, DeEncodedLength, DeTypeCode};
use crate::legacy::data_elements::{
    DataElementDeserializeError, DataElementSerializationBuffer, DataElementSerializeError,
    DeserializeDataElement, DirectMapPredicate, DirectMapper, LengthMapper, SerializeDataElement,
};
use crate::legacy::PacketFlavor;
use crate::private::Sealed;
use crate::shared_data::CapabilityBitset;
use sink::Sink;

/// Data element holding a [CapabilityBitset].
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct RequirementsDataElement {
    /// The requirements capability bitset.
    pub capability_bitset: CapabilityBitset,
}

impl RequirementsDataElement {
    /// Gets the underlying Requirements value
    pub fn requirements_value(&self) -> u8 {
        self.capability_bitset.into()
    }
}

impl From<CapabilityBitset> for RequirementsDataElement {
    fn from(capability_bitset: CapabilityBitset) -> Self {
        Self { capability_bitset }
    }
}

impl From<RequirementsDataElement> for CapabilityBitset {
    fn from(requirements: RequirementsDataElement) -> Self {
        requirements.capability_bitset
    }
}

impl Sealed for RequirementsDataElement {}

impl<F: PacketFlavor> SerializeDataElement<F> for RequirementsDataElement {
    fn de_type_code(&self) -> DeTypeCode {
        RequirementsDataElement::DE_TYPE_CODE
    }

    fn map_actual_len_to_encoded_len(&self, actual_len: DeActualLength) -> DeEncodedLength {
        <Self as DeserializeDataElement>::LengthMapper::map_actual_len_to_encoded_len(actual_len)
    }

    fn serialize_contents(
        &self,
        sink: &mut DataElementSerializationBuffer,
    ) -> Result<(), DataElementSerializeError> {
        sink.try_extend_from_slice(self.requirements_value().to_be_bytes().as_slice())
            .ok_or(DataElementSerializeError::InsufficientSpace)
    }
}

impl DeserializeDataElement for RequirementsDataElement {
    const DE_TYPE_CODE: DeTypeCode = match DeTypeCode::try_from(0b0111) {
        Ok(t) => t,
        Err(_) => unreachable!(),
    };

    type LengthMapper = DirectMapper<RequirementsLengthPredicate>;

    fn deserialize<F: PacketFlavor>(
        de_contents: &[u8],
    ) -> Result<Self, DataElementDeserializeError> {
        de_contents
            .try_into()
            .ok()
            .and_then(|arr: [u8; 1]| CapabilityBitset::try_from(u8::from_be_bytes(arr)).ok())
            .map(|capability_bitset| Self { capability_bitset })
            .ok_or(DataElementDeserializeError::DeserializeError { de_type: Self::DE_TYPE_CODE })
    }
}

pub(in crate::legacy) struct RequirementsLengthPredicate;

impl DirectMapPredicate for RequirementsLengthPredicate {
    fn is_valid(len: usize) -> bool {
        len == 1
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use crate::legacy::data_elements::de_type::{DeActualLength, DeEncodedLength};
    use crate::legacy::data_elements::requirements::RequirementsDataElement;
    use crate::legacy::data_elements::tests::macros::de_roundtrip_test;
    use crate::legacy::data_elements::{DeserializeDataElement, LengthMapper};
    use crate::legacy::serialize::tests::serialize;
    use crate::legacy::Plaintext;
    use crate::{shared_data, DeLengthOutOfRange};
    use std::panic;

    extern crate std;

    #[test]
    fn actual_length_must_be_1() {
        for l in [0, 2] {
            let actual = DeActualLength::try_from(l).unwrap();
            let _ = panic::catch_unwind(|| {
                <RequirementsDataElement as DeserializeDataElement>::LengthMapper::map_actual_len_to_encoded_len(actual)
            })
                .unwrap_err();
        }

        assert_eq!(
            1,
            <RequirementsDataElement as DeserializeDataElement>::LengthMapper::map_actual_len_to_encoded_len(
                DeActualLength::try_from(1).unwrap(),
            )
                .as_u8()
        )
    }

    #[test]
    fn encoded_length_must_be_1() {
        for l in [0, 2] {
            assert_eq!(
                DeLengthOutOfRange,
                <RequirementsDataElement as DeserializeDataElement>::LengthMapper::map_encoded_len_to_actual_len(
                    DeEncodedLength::try_from(l).unwrap()
                )
                    .unwrap_err()
            )
        }

        assert_eq!(
            1,
            <RequirementsDataElement as DeserializeDataElement>::LengthMapper::map_encoded_len_to_actual_len(
                DeEncodedLength::from(1)
            )
                .unwrap()
                .as_u8()
        );
    }

    #[test]
    fn requirements_de_contents_roundtrip_unencrypted() {
        let reqs = shared_data::CapabilityBitset::try_from(15).unwrap();
        let _ = de_roundtrip_test!(
            RequirementsDataElement,
            Requirements,
            Requirements,
            Plaintext,
            serialize::<Plaintext, _>(&RequirementsDataElement::from(reqs))
        );
    }

    mod coverage_gaming {
        use crate::legacy::data_elements::requirements::RequirementsDataElement;
        use crate::shared_data::CapabilityBitset;
        use alloc::format;

        #[test]
        fn requirements_de() {
            let de = RequirementsDataElement::from(CapabilityBitset::try_from(15).unwrap());
            // debug
            let _ = format!("{de:?}");
            // trivial accessor
            assert_eq!(15, de.requirements_value());
        }
    }
}
