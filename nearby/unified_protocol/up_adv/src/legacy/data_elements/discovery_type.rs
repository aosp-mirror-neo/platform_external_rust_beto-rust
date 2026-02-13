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

//! Data element for Discovery Type.

use crate::legacy::data_elements::de_type::{DeActualLength, DeEncodedLength, DeTypeCode};
use crate::legacy::data_elements::{
    DataElementDeserializeError, DataElementSerializationBuffer, DataElementSerializeError,
    DeserializeDataElement, DirectMapPredicate, DirectMapper, LengthMapper, SerializeDataElement,
};
use crate::legacy::PacketFlavor;
use crate::private::Sealed;
use crate::shared_data::DiscoveryType;
use sink::Sink;

/// Data element holding a [DiscoveryType].
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DiscoveryTypeDataElement {
    /// The discovery type value
    pub discovery_type: DiscoveryType,
}

impl DiscoveryTypeDataElement {
    /// Gets the underlying Discovery Type value
    pub fn discovery_type_value(&self) -> u8 {
        self.discovery_type as u8
    }
}

impl From<DiscoveryType> for DiscoveryTypeDataElement {
    fn from(discovery_type: DiscoveryType) -> Self {
        Self { discovery_type }
    }
}

impl Sealed for DiscoveryTypeDataElement {}

impl<F: PacketFlavor> SerializeDataElement<F> for DiscoveryTypeDataElement {
    fn de_type_code(&self) -> DeTypeCode {
        DiscoveryTypeDataElement::DE_TYPE_CODE
    }

    fn map_actual_len_to_encoded_len(&self, actual_len: DeActualLength) -> DeEncodedLength {
        <Self as DeserializeDataElement>::LengthMapper::map_actual_len_to_encoded_len(actual_len)
    }

    fn serialize_contents(
        &self,
        sink: &mut DataElementSerializationBuffer,
    ) -> Result<(), DataElementSerializeError> {
        sink.try_push(self.discovery_type as u8).ok_or(DataElementSerializeError::InsufficientSpace)
    }
}

impl DeserializeDataElement for DiscoveryTypeDataElement {
    const DE_TYPE_CODE: DeTypeCode = match DeTypeCode::try_from(0b1001) {
        Ok(t) => t,
        Err(_) => unreachable!(),
    };

    type LengthMapper = DirectMapper<DiscoveryTypeLengthPredicate>;

    fn deserialize<F: PacketFlavor>(
        de_contents: &[u8],
    ) -> Result<Self, DataElementDeserializeError> {
        de_contents
            .try_into()
            .ok()
            .and_then(|arr: [u8; 1]| DiscoveryType::try_from(u8::from_be_bytes(arr)).ok())
            .map(|discovery_type| Self { discovery_type })
            .ok_or(DataElementDeserializeError::DeserializeError { de_type: Self::DE_TYPE_CODE })
    }
}

pub(in crate::legacy) struct DiscoveryTypeLengthPredicate;

impl DirectMapPredicate for DiscoveryTypeLengthPredicate {
    fn is_valid(len: usize) -> bool {
        len == 1
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use crate::legacy::data_elements::de_type::{DeActualLength, DeEncodedLength};
    use crate::legacy::data_elements::discovery_type::DiscoveryTypeDataElement;
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
                <DiscoveryTypeDataElement as DeserializeDataElement>::LengthMapper::map_actual_len_to_encoded_len(actual)
            })
                .unwrap_err();
        }

        assert_eq!(
            1,
            <DiscoveryTypeDataElement as DeserializeDataElement>::LengthMapper::map_actual_len_to_encoded_len(
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
                <DiscoveryTypeDataElement as DeserializeDataElement>::LengthMapper::map_encoded_len_to_actual_len(
                    DeEncodedLength::try_from(l).unwrap()
                )
                    .unwrap_err()
            )
        }

        assert_eq!(
            1,
            <DiscoveryTypeDataElement as DeserializeDataElement>::LengthMapper::map_encoded_len_to_actual_len(
                DeEncodedLength::from(1)
            )
                .unwrap()
                .as_u8()
        );
    }

    #[test]
    fn discovery_type_de_contents_roundtrip_unencrypted() {
        let discovery_type = shared_data::DiscoveryType::try_from(1).unwrap();
        let _ = de_roundtrip_test!(
            DiscoveryTypeDataElement,
            DiscoveryType,
            DiscoveryType,
            Plaintext,
            serialize::<Plaintext, _>(&DiscoveryTypeDataElement::from(discovery_type))
        );
    }

    mod coverage_gaming {
        use crate::legacy::data_elements::discovery_type::DiscoveryTypeDataElement;
        use crate::shared_data::DiscoveryType;
        use alloc::format;

        #[test]
        fn discovery_type_de() {
            let de = DiscoveryTypeDataElement::from(DiscoveryType::try_from(1).unwrap());
            // debug
            let _ = format!("{de:?}");
            // trivial accessor
            assert_eq!(1, de.discovery_type_value());
        }
    }
}
