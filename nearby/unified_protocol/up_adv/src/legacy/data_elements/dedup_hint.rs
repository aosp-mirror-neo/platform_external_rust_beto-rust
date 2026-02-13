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

//! Data element for Device Info.

use crate::legacy::data_elements::de_type::{DeActualLength, DeEncodedLength, DeTypeCode};
use crate::legacy::data_elements::{
    DataElementDeserializeError, DataElementSerializationBuffer, DataElementSerializeError,
    DeserializeDataElement, DirectMapPredicate, DirectMapper, LengthMapper, SerializeDataElement,
};
use crate::legacy::PacketFlavor;
use crate::private::Sealed;
use crate::shared_data::{DedupHint, DEDUP_HINT_LENGTH};
use sink::Sink;

/// Information to dedupe advertisements from the same device across different mediums.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeduplicationHintDataElement {
    /// Deduplication hint used to dedupe public advertisements.
    pub dedup_hint: DedupHint,
}

impl From<DedupHint> for DeduplicationHintDataElement {
    fn from(dedup_hint: DedupHint) -> Self {
        Self { dedup_hint }
    }
}

impl Sealed for DeduplicationHintDataElement {}

impl<F: PacketFlavor> SerializeDataElement<F> for DeduplicationHintDataElement {
    fn de_type_code(&self) -> DeTypeCode {
        DeduplicationHintDataElement::DE_TYPE_CODE
    }

    fn map_actual_len_to_encoded_len(&self, actual_len: DeActualLength) -> DeEncodedLength {
        <Self as DeserializeDataElement>::LengthMapper::map_actual_len_to_encoded_len(actual_len)
    }

    fn serialize_contents(
        &self,
        sink: &mut DataElementSerializationBuffer,
    ) -> Result<(), DataElementSerializeError> {
        sink.try_extend_from_slice(&self.dedup_hint.as_bytes())
            .ok_or(DataElementSerializeError::InsufficientSpace)
    }
}

impl DeserializeDataElement for DeduplicationHintDataElement {
    const DE_TYPE_CODE: DeTypeCode = match DeTypeCode::try_from(0b1000) {
        Ok(t) => t,
        Err(_) => unreachable!(),
    };

    type LengthMapper = DirectMapper<DedupHintLengthPredicate>;

    fn deserialize<F: PacketFlavor>(
        de_contents: &[u8],
    ) -> Result<Self, DataElementDeserializeError> {
        de_contents
            .try_into()
            .ok()
            .map(|arr: [u8; DEDUP_HINT_LENGTH]| Self { dedup_hint: DedupHint::from(arr) })
            .ok_or(DataElementDeserializeError::DeserializeError { de_type: Self::DE_TYPE_CODE })
    }
}

pub(in crate::legacy) struct DedupHintLengthPredicate;

impl DirectMapPredicate for DedupHintLengthPredicate {
    fn is_valid(len: usize) -> bool {
        len == 5
    }
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::legacy::data_elements::de_type::{DeActualLength, DeEncodedLength};
    use crate::legacy::data_elements::tests::macros::de_roundtrip_test;
    use crate::legacy::data_elements::{DeserializeDataElement, LengthMapper};
    use crate::legacy::serialize::tests::serialize;
    use crate::legacy::Plaintext;
    use crate::DeLengthOutOfRange;
    use std::panic;

    extern crate std;

    #[test]
    fn actual_length_must_be_5() {
        for l in [0, 2, 4, 6] {
            let actual = DeActualLength::try_from(l).unwrap();
            let _ = panic::catch_unwind(|| {
                <DeduplicationHintDataElement as DeserializeDataElement>::LengthMapper::map_actual_len_to_encoded_len(actual)
            })
                .unwrap_err();
        }

        assert_eq!(
            5,
            <DeduplicationHintDataElement as DeserializeDataElement>::LengthMapper::map_actual_len_to_encoded_len(
                DeActualLength::try_from(5).unwrap(),
            )
                .as_u8()
        )
    }

    #[test]
    fn encoded_length_must_be_5() {
        for l in [0, 2, 4, 6] {
            assert_eq!(
                DeLengthOutOfRange,
                <DeduplicationHintDataElement as DeserializeDataElement>::LengthMapper::map_encoded_len_to_actual_len(
                    DeEncodedLength::try_from(l).unwrap()
                )
                    .unwrap_err()
            )
        }

        assert_eq!(
            5,
            <DeduplicationHintDataElement as DeserializeDataElement>::LengthMapper::map_encoded_len_to_actual_len(
                DeEncodedLength::from(5)
            )
                .unwrap()
                .as_u8()
        );
    }

    #[test]
    fn dedup_hint_de_contents_roundtrip_unencrypted() {
        let dedup_hint = DedupHint::from([1, 2, 3, 4, 5]);
        let _ = de_roundtrip_test!(
            DeduplicationHintDataElement,
            DedupHint,
            DedupHint,
            Plaintext,
            serialize::<Plaintext, _>(&DeduplicationHintDataElement::from(dedup_hint))
        );
    }

    mod coverage_gaming {
        use super::*;
        use alloc::format;

        #[test]
        fn dedup_hint_de() {
            let dedup_hint = DedupHint::from([1, 2, 3, 4, 5]);
            let de = DeduplicationHintDataElement::from(dedup_hint);
            // debug
            let _ = format!("{de:?}");
            // trivial accessor
            assert_eq!(dedup_hint, de.dedup_hint);
        }
    }
}
