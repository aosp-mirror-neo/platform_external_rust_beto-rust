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
    types::{ContentTypeHint, DecodedResult, DeviceUniqueId, EncodingRequest},
};
use common::data_elements::{CoreDataElement, DeviceCapability, ServiceId};
use crypto_provider_default::CryptoProviderImpl;
use identity_common::definitions::{
    DeviceInfo, IdentityProviderConfig, ProviderName, VisibilityLevel, VisibilityScope,
};
use identity_manager::{
    api::IdentityManager, get_identity_manager_with_default_storage, types::StableDeviceId,
};
use identity_provider::fake_provider::FakeIdentityProvider;
use rstest::*;
use std::{collections::HashMap, sync::Arc};

struct TestEnv {
    identity_manager: Arc<dyn IdentityManager>,
    codec: CodecImpl<CryptoProviderImpl>,
}

impl TestEnv {
    async fn new() -> Self {
        #[allow(clippy::unwrap_used)]
        let identity_manager =
            Arc::new(get_identity_manager_with_default_storage::<CryptoProviderImpl>().unwrap());
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let codec = CodecImpl::new(identity_manager.clone());

        assert!(identity_manager
            .update_device_info(DeviceInfo { device_name: "test_device".to_string() })
            .await
            .is_ok());

        Self { identity_manager, codec }
    }
}

async fn register_test_identity_provider(
    provider_name: ProviderName,
    test_env: &mut TestEnv,
) -> Arc<FakeIdentityProvider> {
    let fake_provider = Arc::new(FakeIdentityProvider::new());
    // 1. Setup: register a provider to get credentials
    let config = IdentityProviderConfig {
        provider_name: provider_name.clone(),
        visibility_levels: vec![VisibilityLevel::SelfScope, VisibilityLevel::Contact],
    };
    test_env
        .identity_manager
        .register_identity_provider(Box::new((*fake_provider).clone()), config);

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await; // Give time for creds to generate

    // Simulate remote syncing by grabbing local credentials
    let remote_creds = fake_provider.get_local_credentials();
    let remote_creds_vec = remote_creds.values().flat_map(|v| v.clone()).collect();
    fake_provider.inject_remote_credentials(remote_creds_vec).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    fake_provider
}

#[fixture]
async fn test_env() -> TestEnv {
    TestEnv::new().await
}

#[rstest]
#[tokio::test]
#[allow(clippy::panic, clippy::unwrap_used)]
async fn test_v0_private_encode_decode_roundtrip(#[future] test_env: TestEnv) {
    let mut test_env = test_env.await;
    // Register default identity provider.
    let default_provider_name = ProviderName::default();
    let _fake_provider =
        register_test_identity_provider(default_provider_name.clone(), &mut test_env).await;
    // 2. Create encoding request
    let visibility = VisibilityLevel::SelfScope;
    let data_elements = vec![
        CoreDataElement::BleTxPower(10),
        CoreDataElement::ServiceIds(Vec::from([ServiceId(4), ServiceId(8)])),
    ];
    let request = EncodingRequest::V0 { visibility, data_elements: data_elements.clone() };

    let codec = test_env.codec;
    // 3. Encode
    let result = codec.encode_with_visibility(&request);
    assert!(result.is_ok());
    let service_data = result.unwrap();

    // 4. Decode
    let decode_result =
        codec.decode_with_visibility(service_data.data.as_slice(), ContentTypeHint::Up);
    assert!(decode_result.is_ok());
    let DecodedResult::V0 { decoded_info, device_unique_id, .. } = decode_result.unwrap() else {
        panic!("Expected V0 decoded result")
    };

    // 5. Verify
    assert!(matches!(device_unique_id, DeviceUniqueId::Stable(StableDeviceId(_))));
    assert_eq!(decoded_info.device_name, "test_device".to_string());
    for de in data_elements {
        assert!(decoded_info.data_elements.contains(&de));
    }
}

#[rstest]
#[tokio::test]
#[allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]
async fn test_v1_private_encode_decode_roundtrip(#[future] test_env: TestEnv) {
    let mut test_env = test_env.await;
    // Register default identity provider.
    let default_provider_name = ProviderName::default();
    let _fake_provider =
        register_test_identity_provider(default_provider_name.clone(), &mut test_env).await;

    // 1. Create encoding request for self scope visibility
    let visibility = VisibilityScope::SelfScope { provider: Some(ProviderName::default().0) };
    let data_elements = vec![
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
    let _ = de_map.insert(visibility, data_elements.clone());
    let request = EncodingRequest::V1 { data_elements: de_map };

    let codec = test_env.codec;
    // 3. Encode
    let result = codec.encode_with_visibility(&request);
    assert!(result.is_ok());
    let service_data = result.expect("encoding should succeed");

    // 3. Decode
    let decode_result =
        codec.decode_with_visibility(service_data.data.as_slice(), ContentTypeHint::Up);
    let DecodedResult::V1 { device_unique_id, decoded_info_list, invalid_sections_count } =
        decode_result.unwrap()
    else {
        panic!("Expected V1 decoded result")
    };

    // 5. Verify
    assert!(matches!(device_unique_id, DeviceUniqueId::Stable(StableDeviceId(_))));
    assert_eq!(invalid_sections_count, 0);
    assert_eq!(decoded_info_list.len(), 1);
    let discovered_device_info =
        decoded_info_list.first().expect("decoded_info_list should not be empty");
    assert_eq!(discovered_device_info.device_name, "test_device".to_string());

    for de in data_elements {
        assert!(discovered_device_info.data_elements.contains(&de));
    }
}

#[rstest]
#[tokio::test]
#[allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]
async fn test_v1_multi_provider_encode_decode_roundtrip(#[future] test_env: TestEnv) {
    let mut test_env_lock = test_env.await;

    // Register default provider and non-default ("new") provider.
    let default_provider = ProviderName::default();
    let _fake_provider_default =
        register_test_identity_provider(default_provider.clone(), &mut test_env_lock).await;
    let new_provider = ProviderName("new_provider".to_string());
    let _fake_provider_new =
        register_test_identity_provider(new_provider.clone(), &mut test_env_lock).await;

    // 1. Create V1 encoding request with multiple visibility scopes from multiple providers
    let visibility_1 = VisibilityScope::SelfScope { provider: Some(default_provider.0.clone()) };
    let visibility_2 = VisibilityScope::Contact { provider: Some(default_provider.0) };
    let visibility_3 = VisibilityScope::Contact { provider: Some(new_provider.0) };

    let data_elements = vec![
        CoreDataElement::BleTxPower(10),
        CoreDataElement::ServiceIds(Vec::from([ServiceId(9)])),
        CoreDataElement::BleConnectionInformation {
            mac_address: vec![0, 1, 2, 3, 4, 5],
            gatt_characteristic: vec![6, 7, 8, 9, 10],
            psm: 1234,
            device_token: 5678,
        },
        CoreDataElement::ContextSequenceNumber(5),
    ];

    let mut de_map = HashMap::new();
    let _ = de_map.insert(visibility_1, data_elements.clone());
    let _ = de_map.insert(visibility_2, data_elements.clone());
    let _ = de_map.insert(visibility_3, data_elements.clone());
    let request = EncodingRequest::V1 { data_elements: de_map };

    let codec = test_env_lock.codec;
    // 3. Encode
    let result = codec.encode_with_visibility(&request);
    println!("{result:?}");
    assert!(result.is_ok());
    let service_data = result.expect("encoding should succeed");
    println!("result size: {}", service_data.data.len());

    // 3. Decode
    let decode_result =
        codec.decode_with_visibility(service_data.data.as_slice(), ContentTypeHint::Up);
    let DecodedResult::V1 { device_unique_id, decoded_info_list, invalid_sections_count } =
        decode_result.unwrap()
    else {
        panic!("Expected V1 decoded result")
    };

    // 5. Verify
    assert!(matches!(device_unique_id, DeviceUniqueId::Stable(StableDeviceId(_))));
    assert_eq!(invalid_sections_count, 0);
    assert_eq!(decoded_info_list.len(), 3);
    for discovered_device_info in decoded_info_list {
        assert_eq!(discovered_device_info.device_name, "test_device".to_string());
        for de in data_elements.clone() {
            assert!(discovered_device_info.data_elements.contains(&de));
        }
    }
}
