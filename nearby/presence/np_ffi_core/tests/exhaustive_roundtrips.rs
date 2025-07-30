// Copyright 2024 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Tests that round-trip ser->deser of V1 DEs whose values are exhaustively enumerable is non-panicking and maintains information.

use np_ffi_core::{common::TxPower, deserialize::v1::*, v1::*};

#[test]
fn roundtrip_tx_power() {
    for tx_power in i8::MIN..=i8::MAX {
        let tx_power = TxPower::from_i8_unchecked(tx_power);

        let Ok(de) = tx_power.try_serialize_v1() else {
            // The transmission power was out-of-range on the stack,
            // and we caught that, so we don't care what happens next.
            return;
        };
        assert_eq!(
            de.try_deserialize(),
            Ok(V1DataElementDeserializationResult::Success(V1DataElement::TxPower(tx_power)))
        );
    }
}

#[test]
fn roundtrip_device_type() {
    for device_type in 0..=255u8 {
        let device_type = DeviceType { device_type };
        let de = device_type.serialize();
        assert_eq!(
            de.try_deserialize(),
            Ok(V1DataElementDeserializationResult::Success(V1DataElement::DeviceType(device_type)))
        );
    }
}

#[test]
fn roundtrip_context_sync_seq_num() {
    for context_sync_seq_num in 0..=255u8 {
        let context_sync_seq_num = ContextSyncSeqNum::from_u8_unchecked(context_sync_seq_num);

        let Ok(de) = context_sync_seq_num.try_serialize() else {
            // The context-sync sequence number was out-of-range on the stack,
            // and we caught that, so we don't care what happens next.
            return;
        };
        assert_eq!(
            de.try_deserialize(),
            Ok(V1DataElementDeserializationResult::Success(V1DataElement::ContextSyncSeqNum(
                context_sync_seq_num
            )))
        );
    }
}
