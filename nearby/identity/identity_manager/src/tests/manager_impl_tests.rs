#![allow(clippy::unwrap_used)]
// Copyright 2025 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
//
// you may not use this file except in compliance with the License.
//
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
//
// distributed under the License is distributed on an "AS IS" BASIS,
//
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//
// See the License for the specific language governing permissions and
//
// limitations under the License.
//
//

use crate::{
    api::IdentityManager,
    credentials::{
        credential_manager::CredentialManager, credential_manager_impl::CredentialManagerImpl,
    },
    identity_storage::{IdentityManagerPreferences, PreferencesManager},
    manager_impl::IdentityManagerImpl,
    tests::{fake_credentials::FakeCredentialsStorage, fake_preferences::FakePreferencesManager},
};
use async_trait::async_trait;
use credential_proto::proto::credential::{
    DiscoveryCredential, VisibilityLevel as ProtoVisibilityLevel,
    VisibilityScope as ProtoVisibilityScope,
};
use crypto_provider_default::CryptoProviderImpl;
use identity_common::definitions::{
    DeviceInfo, IdentityProviderConfig, ProviderName, VisibilityLevel, VisibilityScope,
};
use identity_provider::api::{IdentityProvider, IdentityProviderError, IdentityProviderEvent};
use mockall::mock;
use protobuf::{EnumOrUnknown, MessageField};
use rstest::*;
use serial_test::serial;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex as StdMutex},
};
use tokio::sync::{broadcast, mpsc, watch, Mutex};
use up_adv::credential::book::CredentialBook as _;

mock! {
    pub IdentityProvider {}
    #[async_trait]
    impl IdentityProvider for IdentityProvider {
        async fn on_provider_registered(
            &mut self,
            event_sender: mpsc::Sender<IdentityProviderEvent>,
        );

        fn sign(
            &self,
            broadcast_credential_id: i32,
            data: &[u8],
        ) -> Result<Vec<u8>, IdentityProviderError>;

        async fn update_local_discovery_credentials(
            &self,
            local_discovery_credentials: HashMap<VisibilityLevel, Vec<DiscoveryCredential>>,
        );
    }
}

pub struct DroppingIdentityProvider;

#[async_trait]
impl IdentityProvider for DroppingIdentityProvider {
    // This impl will drop the sender channel after 1 sec to simulate the process of an identity provider app unregistering in a prod environment.
    async fn on_provider_registered(&mut self, event_sender: mpsc::Sender<IdentityProviderEvent>) {
        // Logically, a real provider would store this.
        // Our test impl drops it 1 second after the call returns to induce the unregistration logic for testing.
        #[allow(clippy::let_underscore_future)]
        let _ = tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
            drop(event_sender);
        });
    }

    async fn update_local_discovery_credentials(
        &self,
        _local_discovery_credentials: HashMap<VisibilityLevel, Vec<DiscoveryCredential>>,
    ) {
    }

    fn sign(
        &self,
        _broadcast_credential_id: i32,
        _data: &[u8],
    ) -> Result<Vec<u8>, IdentityProviderError> {
        Ok(vec![])
    }
}

struct TestEnv {
    identity_manager: IdentityManagerImpl<CryptoProviderImpl>,
    credential_manager: Arc<CredentialManagerImpl<CryptoProviderImpl>>,
    preferences_reader: watch::Receiver<IdentityManagerPreferences>,
    _remote_discovery_credentials_rx: watch::Receiver<()>,
}

impl TestEnv {
    async fn new() -> Self {
        let (broadcast_credentials_changed, _) = broadcast::channel(10);
        let (local_discovery_credentials_changed, _) = broadcast::channel(10);
        let (remote_discovery_credentials_changed, _remote_discovery_credentials_rx) =
            watch::channel(());
        let preferences_manager = FakePreferencesManager::default();
        let preferences_reader = preferences_manager.subscribe();
        let credential_manager = Arc::new(CredentialManagerImpl::new_for_test(
            preferences_reader.clone(),
            Arc::new(FakeCredentialsStorage::new()),
            broadcast_credentials_changed.clone(),
            local_discovery_credentials_changed.clone(),
            remote_discovery_credentials_changed.clone(),
        ));

        let identity_manager = IdentityManagerImpl::new_for_test(
            Arc::new(Mutex::new(preferences_manager)),
            preferences_reader.clone(),
            credential_manager.clone(),
            local_discovery_credentials_changed,
        );
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        assert!(identity_manager
            .update_device_info(DeviceInfo { device_name: "test_device".to_string() })
            .await
            .is_ok());

        Self {
            identity_manager,
            credential_manager,
            preferences_reader,
            _remote_discovery_credentials_rx,
        }
    }
}

#[fixture]
async fn test_env() -> TestEnv {
    TestEnv::new().await
}

async fn register_test_identity_provider(
    provider_name: ProviderName,
    test_env: &mut TestEnv,
) -> (
    Option<mpsc::Sender<IdentityProviderEvent>>,
    Vec<HashMap<VisibilityLevel, Vec<DiscoveryCredential>>>,
) {
    let identity_manager = &mut test_env.identity_manager;
    let captured_event_sender = Arc::new(std::sync::Mutex::new(None));
    let mut mock_provider = MockIdentityProvider::new();
    let captured_event_sender_clone = captured_event_sender.clone();
    let _ = mock_provider
        .expect_on_provider_registered()
        .withf(move |_event_sender| {
            *captured_event_sender_clone.lock().unwrap() = Some(_event_sender.clone());
            true
        })
        .times(1)
        .return_const(());
    // NOTE: we expect `on_discovery_credentials_updated` to be triggered twice:
    // 1. triggered when the provider is registered.
    // 2. triggered when the IdentityPreferences are created for the first time,
    // as this generates a default device name which changes metadata and leads to local creds update.
    let captured_creds = Arc::new(StdMutex::new(Vec::new()));
    let captured_creds_clone = captured_creds.clone();
    let _ = mock_provider.expect_update_local_discovery_credentials().times(1..).returning(
        move |creds| {
            captured_creds_clone.lock().unwrap().push(creds);
        },
    );

    let config = IdentityProviderConfig {
        provider_name: provider_name.clone(),
        visibility_levels: vec![VisibilityLevel::SelfScope, VisibilityLevel::Contact],
    };
    identity_manager.register_identity_provider(Box::new(mock_provider), config);

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let sender_guard = captured_event_sender.lock().unwrap();
    let creds_guard = captured_creds.lock().unwrap();
    (sender_guard.clone(), creds_guard.clone())
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_register_default_identity_provider(#[future] mut test_env: TestEnv) {
    let test_env = &mut test_env.await;
    let provider_name = ProviderName::default();
    let (sender, local_discovery_creds) =
        register_test_identity_provider(provider_name.clone(), test_env).await;

    assert!(sender.is_some());
    assert_eq!(local_discovery_creds.len(), 1);

    // Check that credentials were generated on demand
    let visibility_scope = VisibilityScope::SelfScope { provider: Some(provider_name.0) };
    let credential_manager = test_env.credential_manager.clone();
    assert!(credential_manager
        .get_v0_broadcast_credential_with_salt(&visibility_scope.level())
        .is_ok());
    assert!(credential_manager.get_v1_broadcast_credential(&visibility_scope).is_ok());
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_unregister_default_identity_provider(#[future] mut test_env: TestEnv) {
    let test_env = &mut test_env.await;
    let provider_name = ProviderName::default();
    let config = IdentityProviderConfig {
        provider_name: provider_name.clone(),
        visibility_levels: vec![VisibilityLevel::SelfScope, VisibilityLevel::Contact],
    };

    let identity_manager = &mut test_env.identity_manager;
    let dropping_provider = DroppingIdentityProvider {};
    identity_manager.register_identity_provider(Box::new(dropping_provider), config);

    // Check that credentials exist before sender is dropped.
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let visibility_level = VisibilityLevel::SelfScope;
    {
        let credential_manager = test_env.credential_manager.clone();
        assert!(credential_manager
            .get_v0_broadcast_credential_with_salt(&visibility_level)
            .is_ok());
    }

    // Check that credentials were wiped out after sender is dropped.
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

    {
        let credential_manager = test_env.credential_manager.clone();
        assert!(credential_manager
            .get_v0_broadcast_credential_with_salt(&visibility_level)
            .is_err());
    }
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_update_device_info(#[future] test_env: TestEnv) {
    let test_env = test_env.await;
    let device_name = "new_device_name".to_string();
    let device_info = DeviceInfo { device_name: device_name.clone() };

    assert!(test_env.identity_manager.update_device_info(device_info).await.is_ok());
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    assert_eq!(test_env.preferences_reader.borrow().device_name, device_name);
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_remote_credentials_synced_event_triggers_update(#[future] mut test_env: TestEnv) {
    let test_env = &mut test_env.await;
    let provider_name = ProviderName::default();
    let (sender, _) = register_test_identity_provider(provider_name, test_env).await;
    #[allow(clippy::expect_used)]
    let identity_provider_event_sender = sender.expect("provider should successfully register");
    let event = IdentityProviderEvent::RemoteCredentialsSynced {
        remote_credentials: vec![DiscoveryCredential {
            key_seed: [14; 32].to_vec(),
            identity_token_hmac_key_v0: [15; 32].to_vec(),
            encrypted_metadata_bytes_v0: b"v0_contact_meta2".to_vec(),
            identity_token_short_salt_adv_hmac_key_v1: [16; 32].to_vec(),
            identity_token_extended_salt_adv_hmac_key_v1: [17; 32].to_vec(),
            encrypted_metadata_bytes_v1: b"v1_contact_meta2".to_vec(),
            visibility_scope: MessageField::some(ProtoVisibilityScope {
                level: EnumOrUnknown::new(ProtoVisibilityLevel::VISIBILITY_LEVEL_SELF_GROUP),
                ..Default::default()
            }),
            ..Default::default()
        }],
    };
    identity_provider_event_sender.send(event).await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let remote_creds_book =
        test_env.credential_manager.get_private_group_discovery_credential_book().unwrap();
    assert_eq!(remote_creds_book.v0_iter().count(), 1);
    assert_eq!(remote_creds_book.v1_iter().count(), 1);
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_local_discovery_credentials_changed_event_triggers_update(
    #[future] mut test_env: TestEnv,
) {
    let test_env = &mut test_env.await;
    let provider_name = ProviderName::default();
    let captured_event_sender = Arc::new(std::sync::Mutex::new(None));
    let mut mock_provider = MockIdentityProvider::new();
    let captured_event_sender_clone = captured_event_sender.clone();
    let _ = mock_provider
        .expect_on_provider_registered()
        .withf(move |_event_sender| {
            *captured_event_sender_clone.lock().unwrap() = Some(_event_sender.clone());
            true
        })
        .times(1)
        .return_const(());
    // NOTE: we expect `on_discovery_credentials_updated` to be triggered twice:
    // 1. triggered when the provider is registered.
    // 2. triggered when the IdentityPreferences are created for the first time,
    // as this generates a default device name which changes metadata and leads to local creds update.
    let captured_creds = Arc::new(StdMutex::new(Vec::new()));
    let captured_creds_clone = captured_creds.clone();
    let _ = mock_provider.expect_update_local_discovery_credentials().times(2).returning(
        move |creds| {
            captured_creds_clone.lock().unwrap().push(creds);
        },
    );

    let config = IdentityProviderConfig {
        provider_name: provider_name.clone(),
        visibility_levels: vec![VisibilityLevel::SelfScope, VisibilityLevel::Contact],
    };

    let identity_manager = &mut test_env.identity_manager;
    identity_manager.register_identity_provider(Box::new(mock_provider), config);

    // Induce a self discovery credentials change by changing the encrypted metadata with a new device name:
    let device_name = "new_device_name".to_string();
    let device_info = DeviceInfo { device_name: device_name.clone() };
    assert!(test_env.identity_manager.update_device_info(device_info).await.is_ok());

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let creds_guard = captured_creds.lock().unwrap();
    assert_eq!(creds_guard.len(), 1);
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_rotate_credentials_self_regenerates_device_id(#[future] mut test_env: TestEnv) {
    let test_env = &mut test_env.await;
    let provider_name = ProviderName::default();
    let _ = register_test_identity_provider(provider_name.clone(), test_env).await;

    let initial_device_id = test_env.preferences_reader.borrow().device_id;

    // Rotate self credentials
    assert!(test_env
        .identity_manager
        .rotate_credentials(provider_name.clone(), VisibilityLevel::SelfScope)
        .await
        .is_ok());

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let new_device_id = test_env.preferences_reader.borrow().device_id;
    assert_ne!(
        initial_device_id, new_device_id,
        "Device ID should change after self scope rotation"
    );

    // Verify credentials are valid (replenished)
    let credential_manager = test_env.credential_manager.clone();
    assert!(credential_manager
        .get_v0_broadcast_credential_with_salt(&VisibilityLevel::SelfScope)
        .is_ok());
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_rotate_credentials_non_self_preserves_device_id(#[future] mut test_env: TestEnv) {
    let test_env = &mut test_env.await;
    let provider_name = ProviderName::default();
    let _ = register_test_identity_provider(provider_name.clone(), test_env).await;

    let initial_device_id = test_env.preferences_reader.borrow().device_id;

    // Rotate contacts credentials
    assert!(test_env
        .identity_manager
        .rotate_credentials(provider_name.clone(), VisibilityLevel::Contact)
        .await
        .is_ok());

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let new_device_id = test_env.preferences_reader.borrow().device_id;
    assert_eq!(
        initial_device_id, new_device_id,
        "Device ID should NOT change after non-self scope rotation"
    );

    // Verify credentials are valid (replenished)
    let credential_manager = test_env.credential_manager.clone();
    assert!(credential_manager
        .get_v0_broadcast_credential_with_salt(&VisibilityLevel::Contact)
        .is_ok());
}
