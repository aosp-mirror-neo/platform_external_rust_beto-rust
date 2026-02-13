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
use crate::{
    api::Codec,
    codec_impl::CodecImpl,
    types::{ContentTypeHint, DecodedResult, EncodingRequest},
};
use common::data_elements::{CoreDataElement, DeviceCapability, ServiceId};
use crypto_provider_default::CryptoProviderImpl;
use identity_common::definitions::{DeviceType, VisibilityLevel, VisibilityScope};
use identity_manager::{api::IdentityManager, get_identity_manager_with_default_storage};
use rstest::*;
use std::collections::HashMap;
use std::sync::Arc;
use up_adv::shared_data::MAX_DEVICE_NAME_LEN;

struct TestEnv {
    identity_manager: Arc<dyn IdentityManager>,
    codec: CodecImpl<CryptoProviderImpl>,
}

impl TestEnv {
    fn new() -> Self {
        #[allow(clippy::unwrap_used)]
        let identity_manager =
            Arc::new(get_identity_manager_with_default_storage::<CryptoProviderImpl>().unwrap());
        let codec = CodecImpl::new(identity_manager.clone());

        Self { identity_manager, codec }
    }
}

#[fixture]
fn test_env() -> TestEnv {
    TestEnv::new()
}

#[rstest]
#[tokio::test]
#[allow(clippy::panic, clippy::expect_used)]
async fn test_v0_public_encode_decode_roundtrip(test_env: TestEnv) {
    // 1. Create encoding request for public visibility
    let visibility_level = VisibilityLevel::Public;
    let request = EncodingRequest::V0 {
        visibility: visibility_level,
        data_elements: vec![
            CoreDataElement::BleL2capCocPsm(125),
            CoreDataElement::ServiceIds(Vec::from([ServiceId(9)])),
        ],
    };

    let codec = test_env.codec;
    // 2. Encode
    let result = codec.encode_with_visibility(&request);
    let service_data = result.expect("encoding should succeed");

    // 3. Decode
    let decode_result =
        codec.decode_with_visibility(service_data.data.as_slice(), ContentTypeHint::Up);
    let discovered_device_info = if let Ok(DecodedResult::V0 { decoded_info, .. }) = decode_result {
        decoded_info
    } else {
        panic!("Expected V0 decoded result")
    };

    // 5. Verify
    let (expected_device_name, is_trucated) =
        truncate_device_name(test_env.identity_manager.local_device_name().as_bytes());
    assert_eq!(discovered_device_info.device_name.as_bytes().to_vec(), expected_device_name);
    assert_eq!(discovered_device_info.is_name_truncated, is_trucated);
    assert_eq!(
        discovered_device_info.device_type,
        DeviceType::try_from(device_info::device_type() as u8)
            .expect("device type should be valid")
    );
    if let EncodingRequest::V0 { data_elements, .. } = request {
        let mut service_ids = None;
        let mut psm = None;
        for de in data_elements.iter() {
            match de {
                CoreDataElement::ServiceIds(_) => {
                    service_ids = Some(de);
                }
                CoreDataElement::BleL2capCocPsm(_) => {
                    psm = Some(de);
                }
                _ => panic!("Unexpected data element"),
            }
        }
        assert!(discovered_device_info
            .data_elements
            .contains(service_ids.expect("service_ids should be present")));
        assert!(discovered_device_info.data_elements.contains(psm.expect("psm should be present")));
    }
}

#[rstest]
#[tokio::test]
#[allow(clippy::panic, clippy::expect_used)]
async fn test_v1_public_encode_decode_roundtrip(test_env: TestEnv) {
    // 1. Create encoding request for public visibility
    let mut data_elements = vec![
        CoreDataElement::BleTxPower(10),
        CoreDataElement::ServiceIds(Vec::from([ServiceId(9)])),
        CoreDataElement::BleConnectionInformation {
            mac_address: vec![0, 1, 2, 3, 4, 5],
            gatt_characteristic: vec![6, 7, 8, 9, 10],
            psm: 1234,
            device_token: 5678,
        },
        CoreDataElement::WifiLanIpv4ConnectionInformation {
            ip_address: vec![192, 168, 1, 1],
            port: 8080,
            bssid: Some(vec![11, 12, 13, 14, 15, 16]),
        },
        CoreDataElement::ContextSequenceNumber(5),
        CoreDataElement::GoogleCastId([1; 32]),
        CoreDataElement::MediaDeduplicationId([2; 20]),
        CoreDataElement::Capabilities(vec![DeviceCapability::Camera, DeviceCapability::Speaker]),
    ];
    let mut de_map = HashMap::new();
    let _ = de_map.insert(VisibilityScope::Public, data_elements.clone());
    let request = EncodingRequest::V1 { data_elements: de_map };

    let codec = test_env.codec;
    // 2. Encode
    let result = codec.encode_with_visibility(&request);
    let service_data = result.expect("encoding should succeed");

    // 3. Decode
    let decode_result =
        codec.decode_with_visibility(service_data.data.as_slice(), ContentTypeHint::Up);
    let decoded_info_list = if let Ok(DecodedResult::V1 { decoded_info_list, .. }) = decode_result {
        decoded_info_list
    } else {
        panic!("Expected V1 decoded result")
    };

    // 5. Verify
    assert_eq!(decoded_info_list.len(), 1);
    let discovered_device_info =
        decoded_info_list.first().expect("decoded_info_list should not be empty");
    let expected_device_name = test_env.identity_manager.local_device_name().as_bytes().to_vec();
    assert_eq!(discovered_device_info.device_name.as_bytes().to_vec(), expected_device_name);
    assert!(!discovered_device_info.is_name_truncated);
    assert_eq!(
        discovered_device_info.device_type,
        DeviceType::try_from(device_info::device_type() as u8)
            .expect("device type should be valid")
    );
    // Remove device info from DE vec, as device info is not included in final DE vec field. It is instead a dedicated field in discovered_device_info.
    let _ = data_elements.remove(0);
    for de in data_elements {
        assert!(discovered_device_info.data_elements.contains(&de));
    }
}

/// Truncates the device name to 9 bytes if it's longer, returning the truncated name
/// and a boolean indicating if truncation occurred.
fn truncate_device_name(device_name: &[u8]) -> (Vec<u8>, bool) {
    #[allow(clippy::indexing_slicing)]
    if device_name.len() > MAX_DEVICE_NAME_LEN {
        (device_name[..MAX_DEVICE_NAME_LEN].to_vec(), true)
    } else {
        (device_name.to_vec(), false)
    }
}

#[rstest]
#[tokio::test]
#[allow(clippy::panic, clippy::expect_used)]
async fn test_control_long_format_encode_decode_roundtrip(test_env: TestEnv) {
    // 1. Create encoding request for control advertisement
    let data_elements = vec![
        CoreDataElement::Requirements(vec![DeviceCapability::InternetConnectivity]),
        CoreDataElement::ServiceIds(Vec::from([ServiceId(9)])),
    ];
    let request =
        EncodingRequest::Control { short_format: false, data_elements: data_elements.clone() };

    let codec = test_env.codec;
    // 2. Encode
    let result = codec.encode_with_visibility(&request);
    let service_data = result.expect("encoding should succeed");

    // 3. Decode
    let decode_result =
        codec.decode_with_visibility(service_data.data.as_slice(), ContentTypeHint::Up);
    let decoded_data_elements = if let Ok(DecodedResult::Control { data_elements }) = decode_result
    {
        data_elements
    } else {
        panic!("Expected Control decoded result")
    };

    // Control advertisements don't have device info, so we only check data elements.
    for de in data_elements {
        assert!(decoded_data_elements.contains(&de));
    }
}

#[rstest]
#[tokio::test]
#[allow(clippy::panic, clippy::expect_used)]
async fn test_control_short_format_encode_decode_roundtrip(test_env: TestEnv) {
    // 1. Create encoding request for control advertisement
    let data_elements = vec![
        CoreDataElement::Requirements(vec![DeviceCapability::InternetConnectivity]),
        CoreDataElement::ServiceIds(Vec::from([ServiceId(9)])),
    ];
    let request =
        EncodingRequest::Control { short_format: true, data_elements: data_elements.clone() };

    let codec = test_env.codec;
    // 2. Encode
    let result = codec.encode_with_visibility(&request);
    let service_data = result.expect("encoding should succeed");

    // 3. Decode
    let decode_result =
        codec.decode_with_visibility(service_data.data.as_slice(), ContentTypeHint::Up);
    let decoded_data_elements = if let Ok(DecodedResult::Control { data_elements }) = decode_result
    {
        data_elements
    } else {
        panic!("Expected Control decoded result")
    };

    // Control advertisements don't have device info, so we only check data elements.
    for de in data_elements {
        assert!(decoded_data_elements.contains(&de));
    }
}
