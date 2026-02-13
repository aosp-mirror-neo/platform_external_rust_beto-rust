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

#![allow(clippy::unwrap_used)]

extern crate std;

use crate::buffer::StackBuffer;
use alloc::format;
use alloc::vec::Vec;
use rand::rngs::StdRng;
use rand::SeedableRng;
use tinyvec::ArrayVec;

use crypto_provider_default::CryptoProviderImpl;

use crate::extended::data_elements::actions::{ActionId, ActionsDataElement};
use crate::extended::deserialize::data_element::DataElementDeserializationResult;
use crate::extended::serialize::UnencryptedSectionEncoder;
use crate::extended::serialize::{section_tests::SectionBuilderExt, AdvBuilder};
use crate::extended::V1_ENCODING_UNENCRYPTED;

use super::*;

const TEST_BUFFER_SIZE: usize = 250;

#[test]
fn deserialize_cast_id() {
    // Successful case
    let cast_id: [u8; CAST_ID_LENGTH] = [5u8; CAST_ID_LENGTH];
    let de = DataElement::new(CastIdDataElement::DE_TYPE, &cast_id, None);
    let de = DeserializedGoogleDE::try_deserialize(de).expect("Should recognize a cast id.");
    let DeserializedGoogleDE::CastId(de) = de else {
        panic!("Cast Id data elements don't deserialize to Cast Ids?");
    };
    let de = de.expect("A proper-length cast ID should deserialize correctly.");
    assert_eq!(&cast_id, de.get_id_as_bytes());

    // Failure case
    let bad_cast_id: [u8; CAST_ID_LENGTH + 1] = [5u8; CAST_ID_LENGTH + 1];
    let bad_de = DataElement::new(CastIdDataElement::DE_TYPE, &bad_cast_id, None);
    let bad_de = DeserializedGoogleDE::try_deserialize(bad_de)
        .expect("Should recognize even a malformatted cast id.");
    let DeserializedGoogleDE::CastId(bad_de) = bad_de else {
        panic!("Cast Id data elements don't deserialize to Cast Ids?");
    };
    let _ = bad_de.expect_err("Wrong-length cast IDs should yield a deserialization error.");
}

#[test]
fn serialize_cast_id() {
    let cast_id: [u8; CAST_ID_LENGTH] = [5u8; CAST_ID_LENGTH];
    let de = CastIdDataElement::from(&cast_id);
    let mut sink: ArrayVec<[u8; CAST_ID_LENGTH]> = ArrayVec::new();
    de.write_de_contents(Unsalted, &mut sink).expect("Should be able to write a Cast Id.");
    assert_eq!(sink.as_slice(), &cast_id);
}

#[test]
fn deserialize_media_deduplication_id() {
    // Successful case
    let media_deduplication_id: [u8; MEDIA_DEDUPLICATION_ID_LENGTH] =
        [5u8; MEDIA_DEDUPLICATION_ID_LENGTH];
    let de =
        DataElement::new(MediaDeduplicationIdDataElement::DE_TYPE, &media_deduplication_id, None);
    let de = DeserializedGoogleDE::try_deserialize(de)
        .expect("Should recognize a media deduplication id.");
    let DeserializedGoogleDE::MediaDeduplicationId(de) = de else {
        panic!(
            "Media deduplication Id data elements don't deserialize to Media deduplication Ids?"
        );
    };
    let de = de.expect("A proper-length media deduplication ID should deserialize correctly.");
    assert_eq!(&media_deduplication_id, de.get_id_as_bytes());

    // Failure case
    let bad_media_deduplication_id: [u8; MEDIA_DEDUPLICATION_ID_LENGTH + 1] =
        [5u8; MEDIA_DEDUPLICATION_ID_LENGTH + 1];
    let bad_de = DataElement::new(
        MediaDeduplicationIdDataElement::DE_TYPE,
        &bad_media_deduplication_id,
        None,
    );
    let bad_de = DeserializedGoogleDE::try_deserialize(bad_de)
        .expect("Should recognize even a malformatted cast id.");
    let DeserializedGoogleDE::MediaDeduplicationId(bad_de) = bad_de else {
        panic!(
            "Media deduplication Id data elements don't deserialize to Media deduplication Ids?"
        );
    };
    assert_eq!(
        bad_de.expect_err(
            "Wrong-length media deduplication IDs should yield a deserialization error."
        ),
        MediaDeduplicationIdDeserializationError::WrongLength
    );
}

#[test]
fn serialize_media_deduplication_id() {
    let media_deduplication_id: [u8; MEDIA_DEDUPLICATION_ID_LENGTH] =
        [5u8; MEDIA_DEDUPLICATION_ID_LENGTH];
    let de = MediaDeduplicationIdDataElement::from(&media_deduplication_id);
    let mut sink: ArrayVec<[u8; MEDIA_DEDUPLICATION_ID_LENGTH]> = ArrayVec::new();
    de.write_de_contents(Unsalted, &mut sink)
        .expect("Should be able to write a Media deduplication Id.");
    assert_eq!(sink.as_slice(), &media_deduplication_id);
}

#[test]
fn deserialize_broadcast_session_id() {
    // Successful case
    let broadcast_session_id: [u8; BROADCAST_SESSION_ID_LENGTH] =
        [2u8; BROADCAST_SESSION_ID_LENGTH];
    let de = DataElement::new(BroadcastSessionIdDataElement::DE_TYPE, &broadcast_session_id, None);
    let de = DeserializedGoogleDE::try_deserialize(de)
        .expect("Should recognize a broadcast session ID.");
    let DeserializedGoogleDE::BroadcastSessionId(de) = de else {
        panic!("Broadcast Session ID data elements don't deserialize to broadcast sessions?");
    };
    let de = de.expect("A proper-length broadcast session ID should deserialize correctly.");
    assert_eq!(broadcast_session_id, de.broadcast_session_id);

    // Failure case
    let bad_broadcast_session_id: [u8; BROADCAST_SESSION_ID_LENGTH + 2] =
        [2u8; BROADCAST_SESSION_ID_LENGTH + 2];
    let bad_de =
        DataElement::new(BroadcastSessionIdDataElement::DE_TYPE, &bad_broadcast_session_id, None);
    let bad_de = DeserializedGoogleDE::try_deserialize(bad_de)
        .expect("Should recognize even a malformatted broadcast session ID.");
    let DeserializedGoogleDE::BroadcastSessionId(bad_de) = bad_de else {
        panic!("Broadcast Session ID data elements don't deserialize to broadcast sessions?");
    };
    let _ = bad_de.expect_err("Wrong-length broadcast session IDs should fail to deserialize.");
}

#[test]
fn serialize_broadcast_session_id() {
    let broadcast_session_id: [u8; BROADCAST_SESSION_ID_LENGTH] =
        [3u8; BROADCAST_SESSION_ID_LENGTH];
    let de = BroadcastSessionIdDataElement::from(broadcast_session_id);
    let mut sink: ArrayVec<[u8; BROADCAST_SESSION_ID_LENGTH]> = ArrayVec::new();
    de.write_de_contents(Unsalted, &mut sink)
        .expect("Should be able to write a Broadcast Session ID.");
    assert_eq!(sink.as_slice(), &broadcast_session_id);
}

#[test]
fn deserialize_discovery_type() {
    // Success case, with a recognized discovery type.
    let discovery_type = DiscoveryType::Active;
    let discovery_type_bytes: [u8; 1] = [discovery_type as u8];
    let de = DataElement::new(DiscoveryTypeDataElement::DE_TYPE, &discovery_type_bytes, None);
    let de = DeserializedGoogleDE::try_deserialize(de).expect("Should recognize a discovery type.");
    let DeserializedGoogleDE::DiscoveryType(de) = de else {
        panic!("Discovery type data elements don't deserialize to discovery types?");
    };
    let de = de.expect("One byte discovery types should always deserialize.");
    let reconstructed_discovery_type = DiscoveryType::from(de);
    assert_eq!(reconstructed_discovery_type, discovery_type);

    // Failure case: unknown type
    let discovery_type_bytes: [u8; 1] = [42u8];
    let de = DataElement::new(DiscoveryTypeDataElement::DE_TYPE, &discovery_type_bytes, None);
    let de = DeserializedGoogleDE::try_deserialize(de)
        .expect("Should recognize even an out-of-range discovery type.");
    let DeserializedGoogleDE::DiscoveryType(de) = de else {
        panic!("Discovery type data elements don't deserialize to discovery types?");
    };
    let err = de.expect_err("Unknown discovery type should not deserialize.");
    assert_eq!(err, DiscoveryTypeMalformed::UnknownType);

    // Failure case: More than one byte.
    let bad_discovery_type_bytes: [u8; 2] = [1u8, 2u8];
    let de = DataElement::new(DiscoveryTypeDataElement::DE_TYPE, &bad_discovery_type_bytes, None);
    let de = DeserializedGoogleDE::try_deserialize(de)
        .expect("Should recognize even a malformatted discovery type.");
    let DeserializedGoogleDE::DiscoveryType(de) = de else {
        panic!("Discovery type data elements don't deserialize to discovery types?");
    };
    let err = de.expect_err("Discovery types should always be expressed in one byte.");
    assert_eq!(err, DiscoveryTypeMalformed::WrongLength);
}

#[test]
fn serialize_discovery_type() {
    let discovery_type = DiscoveryTypeDataElement::from(DiscoveryType::Ambient);
    let mut sink: ArrayVec<[u8; 1]> = ArrayVec::new();
    discovery_type
        .write_de_contents(Unsalted, &mut sink)
        .expect("Should be able to write a discovery type.");
    assert_eq!(sink.as_slice(), &[DiscoveryType::Ambient as u8]);
}

#[test]
fn deserialize_device_type() {
    // Success case, with a recognized device type.
    let device_type = DeviceType::TV;
    let device_type_bytes: [u8; 1] = [device_type as u8];
    let de = DataElement::new(DeviceTypeDataElement::DE_TYPE, &device_type_bytes, None);
    let de = DeserializedGoogleDE::try_deserialize(de).expect("Should recognize a device type.");
    let DeserializedGoogleDE::DeviceType(de) = de else {
        panic!("Device type data elements don't deserialize to device types?");
    };
    let de = de.expect("One byte device types should always deserialize.");
    let reconstructed_device_type = DeviceType::try_from(de).expect("TV should be a device type.");
    assert_eq!(reconstructed_device_type, device_type);

    // Success case, with an unrecognized device type.
    let device_type_bytes: [u8; 1] = [255u8];
    let de = DataElement::new(DeviceTypeDataElement::DE_TYPE, &device_type_bytes, None);
    let de = DeserializedGoogleDE::try_deserialize(de)
        .expect("Should recognize even an out-of-range device type.");
    let DeserializedGoogleDE::DeviceType(de) = de else {
        panic!("Device type data elements don't deserialize to device types?");
    };
    let de = de.expect("One byte device types should always deserialize.");
    let device_type_val = DeviceType::try_from(de)
        .expect_err("I strongly doubt that there are 255 recognizable kinds of devices.");
    assert_eq!(device_type_val, 255u8);

    // Failure case: More than one byte.
    let bad_device_type_bytes: [u8; 2] = [1u8, 255u8];
    let de = DataElement::new(DeviceTypeDataElement::DE_TYPE, &bad_device_type_bytes, None);
    let de = DeserializedGoogleDE::try_deserialize(de)
        .expect("Should recognize even a malformatted device type.");
    let DeserializedGoogleDE::DeviceType(de) = de else {
        panic!("Device type data elements don't deserialize to device types?");
    };
    let err = de.expect_err("Device types should always be expressed in one byte.");
    assert_eq!(DeviceTypeDeserializationError::WrongLength, err);
}

#[test]
fn serialize_device_type() {
    let device_type = DeviceTypeDataElement::from(DeviceType::Laptop);
    let mut sink: ArrayVec<[u8; 1]> = ArrayVec::new();
    device_type
        .write_de_contents(Unsalted, &mut sink)
        .expect("Should be able to write a device type.");
    assert_eq!(sink.as_slice(), &[DeviceType::Laptop as u8]);
}

#[test]
fn deserialize_device_name() {
    // Success case
    let device_name = "my-device";
    let device_name_bytes = device_name.as_bytes();

    let de = DataElement::new(DeviceNameDataElement::DE_TYPE, device_name_bytes, None);
    let de = DeserializedGoogleDE::try_deserialize(de).expect("Should recognize a device name DE.");
    let DeserializedGoogleDE::DeviceName(de) = de else {
        panic!("Device name data elements don't deserialize to device name?");
    };
    let de = de.expect("Well formed device name DEs should always deserialize.");
    assert_eq!(de.name(), device_name);

    // Success case: valid UTF-8 but long name (up to MAX_DE_LEN)
    let long_name = "a".repeat(100);
    let long_name_bytes = long_name.as_bytes();
    let de = DataElement::new(DeviceNameDataElement::DE_TYPE, long_name_bytes, None);
    let de =
        DeserializedGoogleDE::try_deserialize(de).expect("Should recognize a long device name DE.");
    let DeserializedGoogleDE::DeviceName(de) = de else {
        panic!("Device name data elements don't deserialize to device name?");
    };
    let de = de.expect("Well formed long device name DEs should always deserialize.");
    assert_eq!(de.name(), long_name);

    // Failure case: invalid UTF-8
    let invalid_utf8 = [0xFF, 0xFF, 0xFF];
    let de = DataElement::new(DeviceNameDataElement::DE_TYPE, &invalid_utf8, None);
    let de = DeserializedGoogleDE::try_deserialize(de)
        .expect("Should recognize even a malformatted device name DE.");
    let DeserializedGoogleDE::DeviceName(de) = de else {
        panic!("Device name data elements don't deserialize to device name?");
    };
    let err = de.expect_err("Invalid UTF-8 device names should not deserialize.");
    assert_eq!(DeviceNameDeserializationError::Utf8Error, err);
}

#[test]
fn serialize_device_name() {
    let device_name = "my-laptop";
    let de = DeviceNameDataElement::new(device_name);
    let mut sink: ArrayVec<[u8; 20]> = ArrayVec::new();
    de.write_de_contents(Unsalted, &mut sink).expect("Should be able to write a device name.");
    assert_eq!(sink.as_slice(), device_name.as_bytes());
}

#[test]
fn serialize_tx_power_de() {
    let buffer = StackBuffer::<TEST_BUFFER_SIZE>::new();
    let mut adv_builder = AdvBuilder::<StackBuffer<TEST_BUFFER_SIZE>>::new(buffer);
    let mut section_builder = adv_builder.section_builder(UnencryptedSectionEncoder).unwrap();

    let tx_power = TxPower::try_from(3_i8).unwrap();
    section_builder.add_de(&TxPowerDataElement::from(tx_power)).unwrap();

    assert_eq!(
        &[
            V1_ENCODING_UNENCRYPTED.0,
            2,    // section len
            0x15, // len 1 type 0x05
            3
        ],
        section_builder.into_section::<CryptoProviderImpl>().as_slice()
    );
}

fn actions_ids_from_u16_collection<const N: usize>(actions: [u16; N]) -> ArrayVec<[ActionId; 64]> {
    let mut result = ArrayVec::new();
    result.extend_from_slice(&actions.map(|x| x.try_into().unwrap()));
    result
}

#[test]
fn serialize_actions_de_non_empty() {
    let buffer = StackBuffer::<TEST_BUFFER_SIZE>::new();
    let mut adv_builder = AdvBuilder::<StackBuffer<TEST_BUFFER_SIZE>>::new(buffer);
    let mut section_builder = adv_builder.section_builder(UnencryptedSectionEncoder).unwrap();

    let actions = ActionsDataElement::try_from_actions(actions_ids_from_u16_collection([
        1, 1, 2, 3, 5, 8, // fibonacci, of course
    ]))
    .unwrap();

    section_builder.add_de(&actions).unwrap();

    #[rustfmt::skip]
    assert_eq!(
        &[
            V1_ENCODING_UNENCRYPTED.0,
            7, // section len
            0x66, // len 6 type 0x06
            0b00000100, // container type and len TTLLLLLL
            1, 0, 0, 1, 2 // de-duped delta encoded fibonacci
        ],
        section_builder.into_section::<CryptoProviderImpl>().as_slice()
    );
}

#[rustfmt::skip]
#[test]
fn serialize_context_sync_seq_num_de() {
    let buffer = StackBuffer::<TEST_BUFFER_SIZE>::new();
    let mut adv_builder = AdvBuilder::<StackBuffer<TEST_BUFFER_SIZE>>::new(buffer);
    let mut section_builder =
        adv_builder.section_builder(UnencryptedSectionEncoder).unwrap();

    let de = ContextSyncSeqNum::try_from(3).unwrap();
    let de = ContextSyncSeqNumDataElement::from(de);
    section_builder.add_de(&de).unwrap();

    assert_eq!(
        &[
            V1_ENCODING_UNENCRYPTED.0,
            3, // section len
            0x81, 0x13, // len 1 type 0x13
            3,    // seq num
        ],
        section_builder.into_section::<CryptoProviderImpl>().as_slice()
    );
}

fn random_ble_connectivity_info<R: rand::Rng>(rng: &mut R) -> BleConnectivityInfo {
    let mut result = BleConnectivityInfo::default();
    if rng.gen_bool(0.5) {
        result.mac_address = Some(rand_ext::random_bytes_rc(rng));
    }
    if rng.gen_bool(0.5) {
        let len = rng.gen_range(0..=MAX_GATT_SERVICE_IDENTIFIER_LEN);
        let contents = rand_ext::random_bytes_rc(rng);
        result.gatt_service_identifier = Some(tinyvec::ArrayVec::from_array_len(contents, len));
    }
    if rng.gen_bool(0.5) {
        result.psm = Some(rand_ext::random_bytes_rc(rng));
    }
    if rng.gen_bool(0.5) {
        result.device_token = Some(rand_ext::random_bytes_rc(rng));
    }
    result
}

fn random_wifi_lan_connectivity_info<R: rand::Rng>(rng: &mut R) -> WifiLanConnectivityInfo {
    let mut result = WifiLanConnectivityInfo::default();
    match rng.gen_range(0..=2) {
        1 => {
            let addr: u32 = rng.gen();
            result.ip = Some(core::net::Ipv4Addr::from(addr).into());
        }
        2 => {
            let addr: u128 = rng.gen();
            result.ip = Some(core::net::Ipv6Addr::from(addr).into());
        }
        _ => {}
    }
    if rng.gen_bool(0.5) {
        result.port = Some(rng.gen());
    }
    if rng.gen_bool(0.5) {
        result.bssid = Some(rand_ext::random_bytes_rc(rng));
    }
    result
}

fn test_connectivity_info_component_roundtrip<
    T: ConnectivityInfoComponent + Eq + core::fmt::Debug,
>(
    input: T,
) {
    if input.is_some() {
        let mut serialized = Vec::new();
        serialized.push(input.field_bitmask());
        input
            .serialize_field_values_into_payload(&mut serialized)
            .expect("Should be able to serialize nonempty connectivity info components.");
        let serialized = serialized;

        let (remaining, output) = T::parse_from_payload(&serialized)
            .expect("Serializable connectivity info components should be deserializable.");
        let empty_slice: &[u8] = &[];
        assert_eq!(
            remaining, empty_slice,
            "Deserializing serialized connectivity info components should take all bytes."
        );
        assert_eq!(input, output, "Serialization->Deserialization round trip for connectivity info components should be the identity.");
    }
}

#[test]
fn roundtrip_ble_connectivity_info() {
    let mut rng = StdRng::from_entropy();
    for _ in 0..10_000 {
        let input = random_ble_connectivity_info(&mut rng);
        test_connectivity_info_component_roundtrip(input);
    }
}

#[test]
fn roundtrip_wifi_lan_connectivity_info() {
    let mut rng = StdRng::from_entropy();
    for _ in 0..10_000 {
        let input = random_wifi_lan_connectivity_info(&mut rng);
        test_connectivity_info_component_roundtrip(input);
    }
}

#[test]
fn roundtrip_connectivity_info() {
    let mut rng = StdRng::from_entropy();
    for _ in 0..10_000 {
        let input = ConnectivityInfoDataElement {
            ble_info: random_ble_connectivity_info(&mut rng),
            wifi_lan_info: random_wifi_lan_connectivity_info(&mut rng),
        };

        if !input.is_some() {
            continue;
        }

        let mut serialized = Vec::new();
        input
            .write_de_contents(Unsalted, &mut serialized)
            .expect("Should be able to serialize nonempty connectivity info");
        let serialized = serialized;

        let output = ConnectivityInfoDataElement::try_deserialize(None, &serialized)
            .expect("Should be able to successfully deserialize serialized connectivity info");
        assert_eq!(
            input, output,
            "Connectivity info serialization->deserialization round-trip should be the identity."
        );
    }
}

#[test]
fn connectivity_info_trailing_mediums_ignored() {
    for extra_medium in 2..=255u8 {
        let input = ConnectivityInfoDataElement {
            ble_info: BleConnectivityInfo { psm: Some([0u8, 255u8]), ..Default::default() },
            wifi_lan_info: WifiLanConnectivityInfo { port: Some(u16::MAX), ..Default::default() },
        };

        let mut buffer = Vec::new();
        input.write_de_contents(Unsalted, &mut buffer).unwrap();
        buffer.push(extra_medium);
        // Fake medium contents.
        buffer.extend_from_slice(&[1u8; 10]);
        let buffer = buffer;

        let output = ConnectivityInfoDataElement::try_deserialize(None, &buffer)
            .expect("Connectivity info should be forward-extensible in deserialization");
        assert_eq!(
            input, output,
            "Connectivity info forward-extensibility should not affect known components"
        );
    }
}

#[test]
fn connectivity_info_no_components() {
    let err = ConnectivityInfoDataElement::try_deserialize(None, &[])
        .expect_err("Empty connectivity info should not successfully deserialize.");
    assert_eq!(err, ConnectivityInfoDeserializationError::NoComponents);
}

#[test]
fn connectivity_info_duplicate_components() {
    let ble_info = BleConnectivityInfo { psm: Some([1u8, 2u8]), ..Default::default() };

    let mut buffer = Vec::new();
    ble_info.serialize_if_populated(&mut buffer).unwrap();
    ble_info.serialize_if_populated(&mut buffer).unwrap();
    let buffer = buffer;

    let err = ConnectivityInfoDataElement::try_deserialize(None, &buffer).expect_err(
        "Duplicate connectivity info components should yield an error on deserialization",
    );
    assert_eq!(err, ConnectivityInfoDeserializationError::DuplicateComponents);
}

#[test]
fn connectivity_info_components_out_of_order() {
    let ble_info = BleConnectivityInfo { psm: Some([1u8, 2u8]), ..Default::default() };
    let wifi_lan_info = WifiLanConnectivityInfo { port: Some(16u16), ..Default::default() };

    let mut buffer = Vec::new();
    wifi_lan_info.serialize_if_populated(&mut buffer).unwrap();
    ble_info.serialize_if_populated(&mut buffer).unwrap();
    let buffer = buffer;

    let err = ConnectivityInfoDataElement::try_deserialize(None, &buffer).expect_err(
        "Out-of-order connectivity info components should yield an error on deserialization",
    );
    assert_eq!(err, ConnectivityInfoDeserializationError::ComponentsOutOfOrder);
}

#[test]
fn connectivity_info_deserialize_empty_component() {
    // Wifi-LAN with empty bitmask.
    let buffer = [1u8, 0u8];

    let err = ConnectivityInfoDataElement::try_deserialize(None, &buffer).expect_err(
        "Connectivity info with an empty component should yield a deserialization error.",
    );
    assert_eq!(err, ConnectivityInfoDeserializationError::ComponentParseFailure(1));
}

#[test]
fn wifi_connectivity_info_no_ipv4_and_ipv6() {
    let contents = [
        0b11000000, // Bitmask selecting ipv4 and ipv6.
        192u8, 168u8, 1u8, 1u8, // Test Ipv4 address
        1u8, 2u8, 3u8, 4u8, 5u8, 6u8, 7u8, 8u8, 9u8, 10u8, 11u8, 12u8, 13u8, 14u8, 15u8,
        16u8, // Test Ipv6 address
    ];
    let _ = WifiLanConnectivityInfo::parse_from_payload(&contents)
        .expect_err("Should not be able to specify both IPv4 and Ipv6 in WifiLanConnectivityInfo");
}

#[test]
fn ble_connectivity_info_no_disallowed_bitmasks() {
    // The top four bits of the BLE mask are taken, and the bottom
    // four bits are explicitly disallowed.
    for disallowed_bitmask in 1u8..=15u8 {
        let _ = BleConnectivityInfo::parse_from_payload(&[disallowed_bitmask])
            .expect_err(&format!("Bitmask {disallowed_bitmask} should be disallowed"));
    }
}

#[test]
fn wifi_lan_connectivity_info_no_disallowed_bitmasks() {
    // The top four bits of the Wifi-lan mask are taken, and the bottom
    // four bits are explicitly disallowed.
    for disallowed_bitmask in 1u8..=15u8 {
        let _ = WifiLanConnectivityInfo::parse_from_payload(&[disallowed_bitmask])
            .expect_err(&format!("Bitmask {disallowed_bitmask} should be disallowed"));
    }
}

#[test]
fn capabilities_bitmask() {
    let mut caps = CapabilitiesDataElement::new();
    assert_eq!(caps.capability_bitset.0, 0b0000_0000);

    caps.capability_bitset.add(DeviceCapability::InternetConnectivity);
    assert_eq!(caps.capability_bitset.0, 0b0000_0001);
    assert!(caps.capability_bitset.has(DeviceCapability::InternetConnectivity));
    assert!(!caps.capability_bitset.has(DeviceCapability::Speaker));

    caps.capability_bitset.add(DeviceCapability::Speaker);
    assert_eq!(caps.capability_bitset.0, 0b0000_1001);
    assert!(caps.capability_bitset.has(DeviceCapability::InternetConnectivity));
    assert!(caps.capability_bitset.has(DeviceCapability::Speaker));

    let byte: u8 = caps.capability_bitset.into();
    assert_eq!(byte, 0b0000_1001);
}

#[test]
fn capabilities_from_capability_bitset() {
    let bitset = CapabilityBitset::try_from(0b0000_1101).unwrap();
    let caps = CapabilitiesDataElement::from(bitset);
    assert!(caps.capability_bitset.has(DeviceCapability::InternetConnectivity));
    assert!(!caps.capability_bitset.has(DeviceCapability::CastReceiver));
    assert!(caps.capability_bitset.has(DeviceCapability::Camera));
    assert!(caps.capability_bitset.has(DeviceCapability::Speaker));
}

#[test]
fn deserialize_capabilities_de() {
    let caps = CapabilitiesDataElement::try_deserialize(None, &[0b0000_0101]).unwrap();
    assert!(caps.capability_bitset.has(DeviceCapability::InternetConnectivity));
    assert!(!caps.capability_bitset.has(DeviceCapability::CastReceiver));
    assert!(caps.capability_bitset.has(DeviceCapability::Camera));
    assert!(!caps.capability_bitset.has(DeviceCapability::Speaker));
}

#[test]
fn deserialize_capabilities_de_invalid_length() {
    let err = CapabilitiesDataElement::try_deserialize(None, &[0b0, 0b1]).unwrap_err();
    assert_eq!(err, CapabilityBitsetDeserializationError::InvalidLength);
    let err = CapabilitiesDataElement::try_deserialize(None, &[]).unwrap_err();
    assert_eq!(err, CapabilityBitsetDeserializationError::InvalidLength);
}

#[test]
fn deserialize_capabilities_de_reserved_bits() {
    let err = CapabilitiesDataElement::try_deserialize(None, &[0b1000_0001]).unwrap_err();
    assert_eq!(err, CapabilityBitsetDeserializationError::ReservedBitsSet);
}

#[test]
fn requirements_bitmask() {
    let mut reqs = RequirementsDataElement::new();
    assert_eq!(reqs.capability_bitset.0, 0b0000_0000);

    reqs.capability_bitset.add(DeviceCapability::InternetConnectivity);
    assert_eq!(reqs.capability_bitset.0, 0b0000_0001);
    assert!(reqs.capability_bitset.has(DeviceCapability::InternetConnectivity));
    assert!(!reqs.capability_bitset.has(DeviceCapability::Speaker));

    reqs.capability_bitset.add(DeviceCapability::Speaker);
    assert_eq!(reqs.capability_bitset.0, 0b0000_1001);
    assert!(reqs.capability_bitset.has(DeviceCapability::InternetConnectivity));
    assert!(reqs.capability_bitset.has(DeviceCapability::Speaker));

    let byte: u8 = reqs.capability_bitset.into();
    assert_eq!(byte, 0b0000_1001);
}

#[test]
fn requirements_from_capability_bitset() {
    let bitset = CapabilityBitset::try_from(0b0000_1101).unwrap();
    let reqs = RequirementsDataElement::from(bitset);
    assert!(reqs.capability_bitset.has(DeviceCapability::InternetConnectivity));
    assert!(!reqs.capability_bitset.has(DeviceCapability::CastReceiver));
    assert!(reqs.capability_bitset.has(DeviceCapability::Camera));
    assert!(reqs.capability_bitset.has(DeviceCapability::Speaker));
}

#[test]
fn deserialize_requirements_de() {
    let reqs = RequirementsDataElement::try_deserialize(None, &[0b0000_0101]).unwrap();
    assert!(reqs.capability_bitset.has(DeviceCapability::InternetConnectivity));
    assert!(!reqs.capability_bitset.has(DeviceCapability::CastReceiver));
    assert!(reqs.capability_bitset.has(DeviceCapability::Camera));
    assert!(!reqs.capability_bitset.has(DeviceCapability::Speaker));
}

#[test]
fn deserialize_requirements_de_invalid_length() {
    let err = RequirementsDataElement::try_deserialize(None, &[0b0, 0b1]).unwrap_err();
    assert_eq!(err, CapabilityBitsetDeserializationError::InvalidLength);
    let err = RequirementsDataElement::try_deserialize(None, &[]).unwrap_err();
    assert_eq!(err, CapabilityBitsetDeserializationError::InvalidLength);
}

#[test]
fn deserialize_requirements_de_reserved_bits() {
    let err = RequirementsDataElement::try_deserialize(None, &[0b1000_0001]).unwrap_err();
    assert_eq!(err, CapabilityBitsetDeserializationError::ReservedBitsSet);
}

mod coverage_gaming {
    use alloc::format;

    use super::*;

    #[test]
    fn de_type_const_from() {
        let _ = DeType::const_from(3);
    }

    #[test]
    fn template() {}

    #[test]
    fn generic_de_error_derives() {
        let err = GenericDataElementError::DataTooLong;
        let _ = format!("{err:?}");
        assert_eq!(err, err);
    }

    #[test]
    fn generic_data_element_debug() {
        let generic =
            GenericDataElement::try_from(DeType::from(1000_u16), &[10, 11, 12, 13]).unwrap();
        let _ = format!("{generic:?}");
    }
}
