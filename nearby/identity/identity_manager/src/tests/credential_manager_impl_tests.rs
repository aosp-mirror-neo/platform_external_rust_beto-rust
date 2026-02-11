#![allow(clippy::unwrap_used, clippy::panic)]
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

use super::{fake_credentials::FakeCredentialsStorage, fake_preferences::FakePreferencesManager};
use crate::{
    credentials::{
        credential_manager::{CredentialManager as _, LocalDiscoveryCredentials},
        credential_manager_impl::{CredentialManagerImpl, EXPECTED_NUM_LOCAL_CREDENTIALS},
    },
    identity_storage::{credentials_database::CredentialsDatabase, PreferencesManager},
};
use chrono::Utc;
use credential_proto::proto::{
    broadcast_credential::BroadcastCredential,
    credential::{
        DiscoveryCredential, VisibilityLevel as ProtoVisibilityLevel,
        VisibilityScope as ProtoVisibilityScope,
    },
    credential_book::{BroadcastCredentials, LocalCredentialBook, RemoteCredentialBook},
};
use crypto_provider_default::CryptoProviderImpl;
use identity_common::definitions::{ProviderName, VisibilityLevel, VisibilityScope};
use ldt_np_adv::{V0_IDENTITY_TOKEN_LEN, V0_SALT_LEN};
use protobuf::{EnumOrUnknown, MessageField};
use rstest::*;
use serial_test::serial;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, watch};
use up_adv::{credential::book::CredentialBook as _, extended::V1_IDENTITY_TOKEN_LEN};

fn mock_provider2() -> ProviderName {
    ProviderName("test_provider2".to_string())
}

fn mock_default_provider() -> ProviderName {
    ProviderName::default()
}

fn self_key_seed() -> [u8; 32] {
    [1; 32]
}

fn contacts_key_seed() -> [u8; 32] {
    [2; 32]
}

fn self_remote_key_seed1() -> [u8; 32] {
    [0; 32]
}

fn self_remote_key_seed2() -> [u8; 32] {
    [4; 32]
}

fn contacts_remote_key_seed1() -> [u8; 32] {
    [10; 32]
}

fn contacts_remote_key_seed2() -> [u8; 32] {
    [14; 32]
}

/// Creates a dummy credential pair with only the time fields populated.
fn create_fake_credential_pair(
    start_time_millis: i64,
    end_time_millis: i64,
) -> (BroadcastCredential, DiscoveryCredential) {
    let broadcast_credential = BroadcastCredential {
        identity_token_v0: vec![101; V0_IDENTITY_TOKEN_LEN],
        identity_token_v1: vec![2; V1_IDENTITY_TOKEN_LEN],
        start_time_millis,
        end_time_millis,
        credential_version: 3,
        ..Default::default()
    };
    let discovery_credential = DiscoveryCredential {
        identity_token_hmac_key_v0: [1; 32].to_vec(),
        encrypted_metadata_bytes_v0: b"v0_meta".to_vec(),
        identity_token_short_salt_adv_hmac_key_v1: [2; 32].to_vec(),
        identity_token_extended_salt_adv_hmac_key_v1: [3; 32].to_vec(),
        encrypted_metadata_bytes_v1: b"v1_meta".to_vec(),
        start_time_millis,
        end_time_millis,
        credential_version: 3,
        ..Default::default()
    };
    (broadcast_credential, discovery_credential)
}

fn mock_remote_credentials(
    key_seed: [u8; 32],
    level: ProtoVisibilityLevel,
    id: i64,
) -> [DiscoveryCredential; 1] {
    let (
        identity_token_hmac_key_v0,
        encrypted_metadata_bytes_v0,
        identity_token_short_salt_adv_hmac_key_v1,
        identity_token_extended_salt_adv_hmac_key_v1,
        encrypted_metadata_bytes_v1,
    ) = match level {
        ProtoVisibilityLevel::VISIBILITY_LEVEL_SELF_GROUP => (
            [1; 32].to_vec(),
            b"v0_meta".to_vec(),
            [2; 32].to_vec(),
            [3; 32].to_vec(),
            b"v1_meta".to_vec(),
        ),
        ProtoVisibilityLevel::VISIBILITY_LEVEL_CONTACTS_GROUP => (
            [11; 32].to_vec(),
            b"v0_contact_meta".to_vec(),
            [12; 32].to_vec(),
            [13; 32].to_vec(),
            b"v1_contact_meta".to_vec(),
        ),
        _ => panic!("unsupported visibility level for mock"),
    };
    [DiscoveryCredential {
        id,
        key_seed: key_seed.to_vec(),
        identity_token_hmac_key_v0,
        encrypted_metadata_bytes_v0,
        identity_token_short_salt_adv_hmac_key_v1,
        identity_token_extended_salt_adv_hmac_key_v1,
        encrypted_metadata_bytes_v1,
        visibility_scope: MessageField::some(ProtoVisibilityScope {
            level: EnumOrUnknown::new(level),
            ..Default::default()
        }),
        ..Default::default()
    }]
}

struct TestEnv {
    manager: CredentialManagerImpl<CryptoProviderImpl>,
    credentials_storage: Arc<dyn CredentialsDatabase>,
    // The FakePreferencesManager must be kept alive for the duration of the test,
    // otherwise the watch::Sender is dropped and the CredentialManager will not
    // receive preference updates.
    preferences: FakePreferencesManager,
    // TODO: add test cases which test broadcast credential updates.
    _broadcast_credentials_changed: broadcast::Receiver<VisibilityScope>,
    local_discovery_credentials_changed: broadcast::Receiver<LocalDiscoveryCredentials>,
    remote_discovery_credentials_changed: watch::Receiver<()>,
}

impl TestEnv {
    fn new() -> Self {
        let credentials_storage = Arc::new(FakeCredentialsStorage::new());
        let now = Utc::now().timestamp_millis();
        let (mut self_broadcast_cred, mut self_discovery_cred) =
            create_fake_credential_pair(now, now + 1000 * 60 * 60 * 24);
        self_broadcast_cred.key_seed = self_key_seed().into();
        self_discovery_cred.key_seed = self_key_seed().into();
        self_discovery_cred.visibility_scope = MessageField::some(ProtoVisibilityScope {
            level: EnumOrUnknown::new(ProtoVisibilityLevel::VISIBILITY_LEVEL_SELF_GROUP),
            ..Default::default()
        });
        let mut contacts_broadcast_cred = self_broadcast_cred.clone();
        let mut contacts_discovery_cred = self_discovery_cred.clone();
        contacts_broadcast_cred.key_seed = contacts_key_seed().into();
        contacts_discovery_cred.key_seed = contacts_key_seed().into();
        contacts_discovery_cred.visibility_scope = MessageField::some(ProtoVisibilityScope {
            level: EnumOrUnknown::new(ProtoVisibilityLevel::VISIBILITY_LEVEL_CONTACTS_GROUP),
            ..Default::default()
        });

        // Add local creds to default provider.
        let mut default_local_creds = HashMap::new();
        let _ = default_local_creds.insert(
            VisibilityLevel::SelfScope as i32,
            BroadcastCredentials {
                broadcast_credentials: vec![self_broadcast_cred.clone()],
                converted_discovery_credentials: vec![self_discovery_cred.clone()],
                ..Default::default()
            },
        );

        let _ = default_local_creds.insert(
            VisibilityLevel::Contact as i32,
            BroadcastCredentials {
                broadcast_credentials: vec![contacts_broadcast_cred],
                converted_discovery_credentials: vec![contacts_discovery_cred],
                ..Default::default()
            },
        );
        credentials_storage
            .insert_local_credentials(
                &mock_default_provider(),
                LocalCredentialBook {
                    local_credentials: default_local_creds,
                    ..Default::default()
                },
            )
            .unwrap();

        // Add local credentials for the second mock provider
        let mut local_creds_2 = HashMap::new();
        let _ = local_creds_2.insert(
            VisibilityLevel::SelfScope as i32,
            BroadcastCredentials {
                broadcast_credentials: vec![self_broadcast_cred],
                converted_discovery_credentials: vec![self_discovery_cred],
                ..Default::default()
            },
        );
        credentials_storage
            .insert_local_credentials(
                &mock_provider2(),
                LocalCredentialBook { local_credentials: local_creds_2, ..Default::default() },
            )
            .unwrap();

        // Add remote credentials for both providers.
        credentials_storage
            .insert_remote_credentials(
                &mock_default_provider(),
                RemoteCredentialBook {
                    self_remote_credentials: mock_remote_credentials(
                        self_remote_key_seed1(),
                        ProtoVisibilityLevel::VISIBILITY_LEVEL_SELF_GROUP,
                        1,
                    )
                    .to_vec(),
                    contacts_remote_credentials: mock_remote_credentials(
                        contacts_remote_key_seed1(),
                        ProtoVisibilityLevel::VISIBILITY_LEVEL_CONTACTS_GROUP,
                        2,
                    )
                    .to_vec(),
                    ..Default::default()
                },
            )
            .unwrap();
        credentials_storage
            .insert_remote_credentials(
                &mock_provider2(),
                RemoteCredentialBook {
                    self_remote_credentials: mock_remote_credentials(
                        self_remote_key_seed2(),
                        ProtoVisibilityLevel::VISIBILITY_LEVEL_SELF_GROUP,
                        3,
                    )
                    .to_vec(),
                    contacts_remote_credentials: mock_remote_credentials(
                        contacts_remote_key_seed2(),
                        ProtoVisibilityLevel::VISIBILITY_LEVEL_CONTACTS_GROUP,
                        4,
                    )
                    .to_vec(),
                    ..Default::default()
                },
            )
            .unwrap();

        let mut provider_visibilities = HashMap::new();
        let _ = provider_visibilities.insert(
            mock_default_provider().0,
            vec![VisibilityLevel::SelfScope as i32, VisibilityLevel::Contact as i32],
        );
        let _ = provider_visibilities.insert(
            mock_provider2().0,
            vec![VisibilityLevel::SelfScope as i32, VisibilityLevel::Contact as i32],
        );
        let mut preferences = FakePreferencesManager::default();
        let _ = preferences.set_provider_visibilities(provider_visibilities);
        let (broadcast_creds_changed_sender, broadcast_creds_changed_receiver) =
            broadcast::channel(1);
        let (local_discovery_creds_changed_sender, local_discovery_creds_changed_receiver) =
            broadcast::channel(1);
        let (remote_discovery_creds_changed_sender, remote_discovery_creds_changed_receiver) =
            watch::channel(());
        let manager: CredentialManagerImpl<CryptoProviderImpl> =
            CredentialManagerImpl::new_for_test(
                preferences.subscribe(),
                credentials_storage.clone(),
                broadcast_creds_changed_sender,
                local_discovery_creds_changed_sender,
                remote_discovery_creds_changed_sender,
            );

        Self {
            manager,
            credentials_storage,
            preferences,
            _broadcast_credentials_changed: broadcast_creds_changed_receiver,
            local_discovery_credentials_changed: local_discovery_creds_changed_receiver,
            remote_discovery_credentials_changed: remote_discovery_creds_changed_receiver,
        }
    }
}

#[fixture]
fn test_env() -> TestEnv {
    TestEnv::new()
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_get_v0_broadcast_credential(test_env: TestEnv) {
    let visibility_level = VisibilityLevel::SelfScope;
    let result = test_env.manager.get_v0_broadcast_credential_with_salt(&visibility_level);
    assert!(result.is_ok(), "expected OK, got {result:?}");
    assert_eq!(result.as_ref().unwrap().0.key_seed, self_key_seed());
    assert_eq!(result.as_ref().unwrap().0.identity_token.as_slice(), &[101; V0_IDENTITY_TOKEN_LEN]);
    assert_eq!(result.as_ref().unwrap().1.bytes().len(), V0_SALT_LEN)
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_get_v1_broadcast_credential(test_env: TestEnv) {
    let visibility_scope: VisibilityScope =
        VisibilityScope::SelfScope { provider: Some(mock_default_provider().clone().0) };
    let cred = test_env.manager.get_v1_broadcast_credential(&visibility_scope);
    assert_eq!(cred.as_ref().unwrap().key_seed, self_key_seed());
    assert_eq!(cred.as_ref().unwrap().identity_token.as_slice(), &[2; V1_IDENTITY_TOKEN_LEN]);
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_get_private_discovery_credential_book_merges_all(test_env: TestEnv) {
    let book = test_env.manager.get_private_group_discovery_credential_book().unwrap();
    println!("V0 count: {}", book.v0_iter().count());
    println!("V1 count: {}", book.v1_iter().count());
    assert_eq!(book.v0_iter().count(), 2);
    assert_eq!(book.v1_iter().count(), 2);
}

#[tokio::test]
#[serial]
async fn test_get_private_discovery_credential_book_empty_if_no_remote_creds() {
    let (broadcast_credentials_change, _) = broadcast::channel(1);
    let (local_discovery_credentials_changed_sender, _) = broadcast::channel(1);
    let (remote_discovery_credentials_changed_sender, _) = watch::channel(());
    let manager: CredentialManagerImpl<CryptoProviderImpl> = CredentialManagerImpl::new_for_test(
        FakePreferencesManager::default().subscribe(),
        Arc::new(FakeCredentialsStorage::new()),
        broadcast_credentials_change,
        local_discovery_credentials_changed_sender,
        remote_discovery_credentials_changed_sender,
    );
    let book = manager.get_private_group_discovery_credential_book().unwrap();
    assert_eq!(book.v0_iter().count(), 0);
    assert_eq!(book.v1_iter().count(), 0);
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_get_private_discovery_credential_book_caching(test_env: TestEnv) {
    // First call, should populate cache
    let book1 = test_env.manager.get_private_group_discovery_credential_book().unwrap();

    // Second call, should hit cache
    let book2 = test_env.manager.get_private_group_discovery_credential_book().unwrap();

    // Check that the cached book is returned
    assert!(Arc::ptr_eq(&book1, &book2));
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_update_remote_credentials_invalidates_cache(test_env: TestEnv) {
    // First call, should populate cache
    let book1 = test_env.manager.get_private_group_discovery_credential_book().unwrap();

    let mut new_creds = mock_remote_credentials(
        self_remote_key_seed1(),
        ProtoVisibilityLevel::VISIBILITY_LEVEL_SELF_GROUP,
        5,
    )
    .to_vec();
    new_creds.extend_from_slice(&mock_remote_credentials(
        contacts_remote_key_seed1(),
        ProtoVisibilityLevel::VISIBILITY_LEVEL_CONTACTS_GROUP,
        6,
    ));

    assert!(test_env
        .manager
        .update_remote_credentials(&mock_default_provider(), new_creds)
        .is_ok());
    let mut remote_creds_changed = test_env.remote_discovery_credentials_changed;
    assert!(remote_creds_changed.changed().await.is_ok());

    // Second call, should miss cache and return a new book
    let book2 = test_env.manager.get_private_group_discovery_credential_book().unwrap();
    assert!(!Arc::ptr_eq(&book1, &book2));

    let remote_creds =
        test_env.credentials_storage.get_remote_credentials(&mock_default_provider()).unwrap();
    assert_eq!(
        remote_creds.self_remote_credentials,
        mock_remote_credentials(
            self_remote_key_seed1(),
            ProtoVisibilityLevel::VISIBILITY_LEVEL_SELF_GROUP,
            5,
        )
        .to_vec()
    );
    assert_eq!(
        remote_creds.contacts_remote_credentials,
        mock_remote_credentials(
            contacts_remote_key_seed1(),
            ProtoVisibilityLevel::VISIBILITY_LEVEL_CONTACTS_GROUP,
            6,
        )
        .to_vec()
    );
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_update_remote_credentials_unknown_identity_type_is_ignored(test_env: TestEnv) {
    let mut new_creds = mock_remote_credentials(
        self_remote_key_seed1(),
        ProtoVisibilityLevel::VISIBILITY_LEVEL_SELF_GROUP,
        7,
    )
    .to_vec();
    new_creds.push(DiscoveryCredential {
        visibility_scope: MessageField::some(ProtoVisibilityScope {
            level: EnumOrUnknown::new(ProtoVisibilityLevel::VISIBILITY_LEVEL_UNSPECIFIED),
            ..Default::default()
        }),
        ..Default::default()
    });

    assert!(test_env
        .manager
        .update_remote_credentials(&mock_default_provider(), new_creds)
        .is_ok());
    let mut remote_creds_changed = test_env.remote_discovery_credentials_changed;
    assert!(remote_creds_changed.changed().await.is_ok());

    let remote_creds =
        test_env.credentials_storage.get_remote_credentials(&mock_default_provider()).unwrap();
    assert_eq!(
        remote_creds.self_remote_credentials,
        mock_remote_credentials(
            self_remote_key_seed1(),
            ProtoVisibilityLevel::VISIBILITY_LEVEL_SELF_GROUP,
            7,
        )
        .to_vec()
    );
    assert!(remote_creds.contacts_remote_credentials.is_empty());
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_wipeout_credentials_invalidates_cache(test_env: TestEnv) {
    // First call, should populate cache
    let book1 = test_env.manager.get_private_group_discovery_credential_book().unwrap();
    assert!(test_env.manager.wipeout_credentials(&mock_default_provider(), None).is_ok()); // Start with no credentials

    // Second call, should miss cache and return a new book
    let book2 = test_env.manager.get_private_group_discovery_credential_book().unwrap();
    assert!(!Arc::ptr_eq(&book1, &book2));

    assert!(test_env.credentials_storage.get_local_credentials(&mock_default_provider()).is_err());
    assert!(test_env.credentials_storage.get_remote_credentials(&mock_default_provider()).is_err());
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_replenish_credentials_from_wiped_out_state_succeeds(test_env: TestEnv) {
    assert!(test_env.manager.wipeout_credentials(&mock_default_provider(), None).is_ok()); // Start with no credentials
    assert!(test_env.credentials_storage.get_local_credentials(&mock_default_provider()).is_err());
    assert!(test_env.credentials_storage.get_remote_credentials(&mock_default_provider()).is_err());

    assert!(test_env.manager.replenish_credentials(&mock_default_provider(), None).is_ok());
    let mut local_creds_changed = test_env.local_discovery_credentials_changed;
    let discovery_creds = local_creds_changed.recv().await.unwrap();
    test_env
        .manager
        .save_primed_credentials(&mock_default_provider(), discovery_creds.0 .1)
        .unwrap();

    let final_creds =
        test_env.credentials_storage.get_local_credentials(&mock_default_provider()).unwrap();
    let self_creds =
        final_creds.local_credentials.get(&(VisibilityLevel::SelfScope as i32)).unwrap();
    assert_eq!(self_creds.broadcast_credentials.len(), EXPECTED_NUM_LOCAL_CREDENTIALS);
    let acquainted_creds =
        final_creds.local_credentials.get(&(VisibilityLevel::Contact as i32)).unwrap();
    assert_eq!(acquainted_creds.broadcast_credentials.len(), EXPECTED_NUM_LOCAL_CREDENTIALS);
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_credential_generation_triggers_local_credentials_changed_event(
    mut test_env: TestEnv,
) {
    assert!(test_env.manager.wipeout_credentials(&mock_default_provider(), None).is_ok()); // Start with no credentials
    assert!(test_env.credentials_storage.get_local_credentials(&mock_default_provider()).is_err());
    assert!(test_env.credentials_storage.get_remote_credentials(&mock_default_provider()).is_err());

    assert!(test_env.manager.replenish_credentials(&mock_default_provider(), None).is_ok());
    let discovery_creds = test_env.local_discovery_credentials_changed.recv().await.unwrap();
    test_env
        .manager
        .save_primed_credentials(&mock_default_provider(), discovery_creds.0 .1.clone())
        .unwrap();
    assert_eq!(discovery_creds.0 .0, mock_default_provider());
    assert_eq!(
        discovery_creds
            .0
             .1
            .local_credentials
            .get(&(VisibilityLevel::SelfScope as i32))
            .unwrap()
            .broadcast_credentials
            .len(),
        EXPECTED_NUM_LOCAL_CREDENTIALS
    );
    assert_eq!(
        discovery_creds
            .0
             .1
            .local_credentials
            .get(&(VisibilityLevel::Contact as i32))
            .unwrap()
            .broadcast_credentials
            .len(),
        EXPECTED_NUM_LOCAL_CREDENTIALS
    );
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_replenish_credentials_with_existing_not_expired(mut test_env: TestEnv) {
    let now = Utc::now().timestamp_millis();
    let mut local_creds = HashMap::new();

    // Create credentials that are valid for several hours to avoid flakiness.
    let (self_broadcast_credentials, self_discovery_credentials): (Vec<_>, Vec<_>) = (0
        ..EXPECTED_NUM_LOCAL_CREDENTIALS)
        .map(|i| {
            let end_time = now + (i as i64 + 1) * 1000 * 60 * 60; // Lasts for hours
            create_fake_credential_pair(now, end_time)
        })
        .unzip();

    let (acquainted_broadcast_credentials, acquainted_discovery_credentials): (Vec<_>, Vec<_>) = (0
        ..2)
        .map(|i| {
            let j: i64 = i;
            let end_time = now + (j + 1) * 1000 * 60 * 60; // Lasts for hours
            create_fake_credential_pair(now, end_time)
        })
        .unzip();

    let _ = local_creds.insert(
        VisibilityLevel::SelfScope as i32,
        BroadcastCredentials {
            broadcast_credentials: self_broadcast_credentials.clone(),
            converted_discovery_credentials: self_discovery_credentials,
            ..Default::default()
        },
    );
    let _ = local_creds.insert(
        VisibilityLevel::Contact as i32,
        BroadcastCredentials {
            broadcast_credentials: acquainted_broadcast_credentials.clone(),
            converted_discovery_credentials: acquainted_discovery_credentials,
            ..Default::default()
        },
    );
    test_env
        .credentials_storage
        .insert_local_credentials(
            &mock_default_provider(),
            LocalCredentialBook { local_credentials: local_creds.clone(), ..Default::default() },
        )
        .unwrap();

    // Verify that the credentials are in storage and are considered valid before replenishing.
    let initial_creds =
        test_env.credentials_storage.get_local_credentials(&mock_default_provider()).unwrap();
    let self_creds_from_storage =
        initial_creds.local_credentials.get(&(VisibilityLevel::SelfScope as i32)).unwrap();
    assert_eq!(self_creds_from_storage.broadcast_credentials, self_broadcast_credentials);
    assert!(self_creds_from_storage.broadcast_credentials.iter().all(|c| c.end_time_millis > now));

    let acquainted_creds_from_storage =
        initial_creds.local_credentials.get(&(VisibilityLevel::Contact as i32)).unwrap();
    assert_eq!(
        acquainted_creds_from_storage.broadcast_credentials,
        acquainted_broadcast_credentials
    );
    assert!(acquainted_creds_from_storage
        .broadcast_credentials
        .iter()
        .all(|c| c.end_time_millis > now));

    // Now, call the function under test.
    assert!(test_env.manager.replenish_credentials(&mock_default_provider(), None).is_ok());
    let discovery_creds = test_env.local_discovery_credentials_changed.recv().await.unwrap();
    test_env
        .manager
        .save_primed_credentials(&mock_default_provider(), discovery_creds.0 .1.clone())
        .unwrap();

    // Verify the final state.
    let final_creds =
        test_env.credentials_storage.get_local_credentials(&mock_default_provider()).unwrap();
    let self_creds =
        final_creds.local_credentials.get(&(VisibilityLevel::SelfScope as i32)).unwrap();
    // Since the initial self credentials were not expired and already at the desired number,
    // they should remain unchanged.
    assert_eq!(self_creds.broadcast_credentials.len(), EXPECTED_NUM_LOCAL_CREDENTIALS);
    assert_eq!(self_creds.broadcast_credentials, self_broadcast_credentials);

    let acquainted_creds =
        final_creds.local_credentials.get(&(VisibilityLevel::Contact as i32)).unwrap();
    // The acquainted credentials had 2, so it should be replenished to NUM_LOCAL_CREDENTIALS.
    assert_eq!(acquainted_creds.broadcast_credentials.len(), EXPECTED_NUM_LOCAL_CREDENTIALS);
    // The original 2 should still be present.
    assert!(acquainted_broadcast_credentials.iter().all(|orig_cred| acquainted_creds
        .broadcast_credentials
        .iter()
        .any(|final_cred| final_cred == orig_cred)));
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_get_v1_broadcast_credential_contacts_scope(test_env: TestEnv) {
    let visibility_scope =
        VisibilityScope::Contact { provider: Some(mock_default_provider().clone().0) };
    let cred = test_env.manager.get_v1_broadcast_credential(&visibility_scope);
    assert!(cred.is_ok());
    assert_eq!(cred.as_ref().unwrap().key_seed, contacts_key_seed());
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_replenish_credentials_with_expired(mut test_env: TestEnv) {
    assert!(test_env.manager.wipeout_credentials(&mock_default_provider(), None).is_ok());

    let now = Utc::now().timestamp_millis();
    let mut local_creds = HashMap::new();

    let (expired_local, expired_shared) = create_fake_credential_pair(now, now - 1000);
    let (not_expired_local, not_expired_shared) = create_fake_credential_pair(now, now + 1000);
    let _ = local_creds.insert(
        VisibilityLevel::SelfScope as i32,
        BroadcastCredentials {
            broadcast_credentials: vec![expired_local, not_expired_local],
            converted_discovery_credentials: vec![expired_shared, not_expired_shared],
            ..Default::default()
        },
    );

    let (expired_contact_local, expired_contact_shared) =
        create_fake_credential_pair(now, now - 2000);
    let _ = local_creds.insert(
        VisibilityLevel::Contact as i32,
        BroadcastCredentials {
            broadcast_credentials: vec![expired_contact_local],
            converted_discovery_credentials: vec![expired_contact_shared],
            ..Default::default()
        },
    );
    test_env
        .credentials_storage
        .insert_local_credentials(
            &mock_default_provider(),
            LocalCredentialBook { local_credentials: local_creds, ..Default::default() },
        )
        .unwrap();

    let expired_creds =
        test_env.credentials_storage.get_local_credentials(&mock_default_provider()).unwrap();
    let expired_self_creds =
        expired_creds.local_credentials.get(&(VisibilityLevel::SelfScope as i32)).unwrap();
    assert_eq!(expired_self_creds.broadcast_credentials.len(), 2);
    let expired_acquainted_creds =
        expired_creds.local_credentials.get(&(VisibilityLevel::Contact as i32)).unwrap();
    assert_eq!(expired_acquainted_creds.broadcast_credentials.len(), 1);

    assert!(test_env.manager.replenish_credentials(&mock_default_provider(), None).is_ok());
    let discovery_creds = test_env.local_discovery_credentials_changed.recv().await.unwrap();
    test_env
        .manager
        .save_primed_credentials(&mock_default_provider(), discovery_creds.0 .1.clone())
        .unwrap();

    let final_creds =
        test_env.credentials_storage.get_local_credentials(&mock_default_provider()).unwrap();
    let self_creds =
        final_creds.local_credentials.get(&(VisibilityLevel::SelfScope as i32)).unwrap();
    assert_eq!(self_creds.broadcast_credentials.len(), EXPECTED_NUM_LOCAL_CREDENTIALS);
    assert_eq!(self_creds.converted_discovery_credentials.len(), EXPECTED_NUM_LOCAL_CREDENTIALS);
    // Check that the expired credential was removed and new ones were added.
    // The one non-expired credential should still be there.
    assert!(self_creds.broadcast_credentials.iter().any(|c| c.end_time_millis == now + 1000));
    // All credentials should have end time >= now
    assert!(self_creds.broadcast_credentials.iter().all(|c| c.end_time_millis >= now));

    let acquainted_creds =
        final_creds.local_credentials.get(&(VisibilityLevel::Contact as i32)).unwrap();
    assert_eq!(acquainted_creds.broadcast_credentials.len(), EXPECTED_NUM_LOCAL_CREDENTIALS);
    assert_eq!(
        acquainted_creds.converted_discovery_credentials.len(),
        EXPECTED_NUM_LOCAL_CREDENTIALS
    );
    assert!(acquainted_creds.broadcast_credentials.iter().all(|c| c.end_time_millis >= now));
}

#[rstest]
#[tokio::test]
#[serial]
#[allow(clippy::indexing_slicing)]
async fn test_refresh_encrypted_metadata_on_device_name_change(mut test_env: TestEnv) {
    let initial_creds =
        test_env.credentials_storage.get_local_credentials(&mock_default_provider()).unwrap();
    let initial_discovery_cred = initial_creds
        .local_credentials
        .get(&(VisibilityLevel::SelfScope as i32))
        .unwrap()
        .converted_discovery_credentials[0]
        .clone();

    // Change the device name, which should trigger a metadata refresh.
    let _ = test_env.preferences.set_device_name("new_device_name".to_string());
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Now that we've confirmed the background task has completed and sent the updates,
    // we can safely check the state of the credentials.
    let updated_creds =
        test_env.credentials_storage.get_local_credentials(&mock_default_provider()).unwrap();
    let updated_discovery_cred = updated_creds
        .local_credentials
        .get(&(VisibilityLevel::SelfScope as i32))
        .unwrap()
        .converted_discovery_credentials[0]
        .clone();

    // The encrypted metadata should have changed.
    assert_ne!(
        initial_discovery_cred.encrypted_metadata_bytes_v0,
        updated_discovery_cred.encrypted_metadata_bytes_v0
    );
    assert_ne!(
        initial_discovery_cred.encrypted_metadata_bytes_v1,
        updated_discovery_cred.encrypted_metadata_bytes_v1
    );
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_get_v0_broadcast_credential_with_invalid_creds_triggers_replenishment(
    test_env: TestEnv,
) {
    assert!(test_env.manager.wipeout_credentials(&mock_default_provider(), None).is_ok()); // Start with no credentials.

    let now = Utc::now().timestamp_millis();
    let mut local_creds = HashMap::new();
    let (expired_local, _) = create_fake_credential_pair(now, now - 1000);
    let _ = local_creds.insert(
        VisibilityLevel::SelfScope as i32,
        BroadcastCredentials { broadcast_credentials: vec![expired_local], ..Default::default() },
    );
    test_env
        .credentials_storage
        .insert_local_credentials(
            &mock_default_provider(),
            LocalCredentialBook { local_credentials: local_creds, ..Default::default() },
        )
        .unwrap();
    let visibility_level = VisibilityLevel::SelfScope;
    let cred = test_env.manager.get_v0_broadcast_credential_with_salt(&visibility_level);

    // The first call should fail because the credentials were invalid and triggered a replenishment.
    assert!(cred.is_err());
    // Wait for replenishment to complete.
    let mut local_creds_changed = test_env.local_discovery_credentials_changed;
    let discovery_creds = local_creds_changed.recv().await.unwrap();
    test_env
        .manager
        .save_primed_credentials(&mock_default_provider(), discovery_creds.0 .1)
        .unwrap();

    // The second call should succeed with a replenished credential.
    let cred = test_env.manager.get_v0_broadcast_credential_with_salt(&visibility_level);
    assert!(cred.is_ok());

    let final_creds =
        test_env.credentials_storage.get_local_credentials(&mock_default_provider()).unwrap();
    let self_creds =
        final_creds.local_credentials.get(&(VisibilityLevel::SelfScope as i32)).unwrap();
    assert_eq!(self_creds.broadcast_credentials.len(), EXPECTED_NUM_LOCAL_CREDENTIALS);
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_notify_contact_credential_used_moves_to_front(test_env: TestEnv) {
    // Initially cache is empty.
    let remote_creds =
        test_env.credentials_storage.get_remote_credentials(&mock_default_provider()).unwrap();
    let contact_cred = remote_creds.contacts_remote_credentials.first().unwrap();
    let contact_cred_id = contact_cred.id;

    // 2. Notify used
    test_env.manager.notify_contact_credential_used(contact_cred_id);

    // 3. Verify it is in the frequent book at index 0 (since it's the only one/first one)
    let book = test_env.manager.get_frequent_contact_discovery_credential_book().unwrap();
    let mut iter = book.v1_iter();
    assert_eq!(iter.next().unwrap().1.id(), contact_cred_id);

    // 4. Add another one to make it interesting
    // provider2 has another contact cred
    let remote_creds_2 =
        test_env.credentials_storage.get_remote_credentials(&mock_provider2()).unwrap();
    let contact_cred_2 = remote_creds_2.contacts_remote_credentials.first().unwrap();
    let contact_cred_2_id = contact_cred_2.id;

    assert_ne!(contact_cred_id, contact_cred_2_id);

    // Notify used for second credential
    test_env.manager.notify_contact_credential_used(contact_cred_2_id);

    // Now book should have cred 2 at index 0
    let book = test_env.manager.get_frequent_contact_discovery_credential_book().unwrap();
    let mut iter = book.v1_iter();
    assert_eq!(iter.next().unwrap().1.id(), contact_cred_2_id); // MRU first
    assert_eq!(iter.next().unwrap().1.id(), contact_cred_id); // LRU second

    // 5. Notify first one again -> should move to front
    test_env.manager.notify_contact_credential_used(contact_cred_id);

    let book = test_env.manager.get_frequent_contact_discovery_credential_book().unwrap();
    let mut iter = book.v1_iter();
    assert_eq!(iter.next().unwrap().1.id(), contact_cred_id); // MRU (moved)
    assert_eq!(iter.next().unwrap().1.id(), contact_cred_2_id); // LRU
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_get_v0_broadcast_credential_no_active_credential(test_env: TestEnv) {
    assert!(test_env.manager.wipeout_credentials(&mock_default_provider(), None).is_ok()); // Start with no credentials

    let now = Utc::now().timestamp_millis();
    let mut local_creds = HashMap::new();
    let (expired_local, _) = create_fake_credential_pair(now - 2000, now - 1000);
    let future_start_time = now + (31 * 60 * 1000);
    let (future_local, _) =
        create_fake_credential_pair(future_start_time, future_start_time + 1000);
    let _ = local_creds.insert(
        VisibilityLevel::SelfScope as i32,
        BroadcastCredentials {
            broadcast_credentials: vec![expired_local, future_local],
            ..Default::default()
        },
    );
    test_env
        .credentials_storage
        .insert_local_credentials(
            &mock_default_provider(),
            LocalCredentialBook { local_credentials: local_creds, ..Default::default() },
        )
        .unwrap();

    let visibility_level = VisibilityLevel::SelfScope;
    let cred = test_env.manager.get_v0_broadcast_credential_with_salt(&visibility_level);
    assert!(cred.is_err());
    let mut local_creds_changed = test_env.local_discovery_credentials_changed;
    let discovery_creds = local_creds_changed.recv().await.unwrap();
    test_env
        .manager
        .save_primed_credentials(&mock_default_provider(), discovery_creds.0 .1)
        .unwrap();

    // A second call should now succeed.
    let cred2 = test_env.manager.get_v0_broadcast_credential_with_salt(&visibility_level);
    assert!(cred2.is_ok());
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_wipeout_credentials_specific_level(test_env: TestEnv) {
    let provider = mock_default_provider();

    // Ensure we start with credentials for both Self and Contact levels
    assert!(test_env
        .manager
        .get_v0_broadcast_credential_with_salt(&VisibilityLevel::SelfScope)
        .is_ok());

    // Wipeout only Self credentials
    assert!(test_env
        .manager
        .wipeout_credentials(&provider, Some(VisibilityLevel::SelfScope))
        .is_ok());

    // Self credentials should be gone from storage
    let local_creds = test_env.credentials_storage.get_local_credentials(&provider).unwrap();
    assert!(!local_creds.local_credentials.contains_key(&(VisibilityLevel::SelfScope as i32)));
    assert!(local_creds.local_credentials.contains_key(&(VisibilityLevel::Contact as i32)));

    // Call get again, it should fail (NotFound)
    assert!(test_env
        .manager
        .get_v0_broadcast_credential_with_salt(&VisibilityLevel::SelfScope)
        .is_err());

    // Explicitly replenish
    assert!(test_env
        .manager
        .replenish_credentials(&provider, Some(VisibilityLevel::SelfScope))
        .is_ok());
    let mut local_creds_changed = test_env.local_discovery_credentials_changed;
    let discovery_creds = local_creds_changed.recv().await.unwrap();
    test_env.manager.save_primed_credentials(&provider, discovery_creds.0 .1).unwrap();

    // Call get again, it should succeed
    assert!(test_env
        .manager
        .get_v0_broadcast_credential_with_salt(&VisibilityLevel::SelfScope)
        .is_ok());

    // Verify it was replenished
    let local_creds_replenished =
        test_env.credentials_storage.get_local_credentials(&provider).unwrap();
    assert!(local_creds_replenished
        .local_credentials
        .contains_key(&(VisibilityLevel::SelfScope as i32)));
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_replenish_credentials_specific_level(test_env: TestEnv) {
    let provider = mock_default_provider();

    // Wipeout all credentials first
    assert!(test_env.manager.wipeout_credentials(&provider, None).is_ok());

    // Replenish only Self credentials
    assert!(test_env
        .manager
        .replenish_credentials(&provider, Some(VisibilityLevel::SelfScope))
        .is_ok());
    let mut local_creds_changed = test_env.local_discovery_credentials_changed;
    let discovery_creds = local_creds_changed.recv().await.unwrap();
    test_env.manager.save_primed_credentials(&provider, discovery_creds.0 .1).unwrap();

    let local_creds = test_env.credentials_storage.get_local_credentials(&provider).unwrap();
    // Self should exist
    assert!(local_creds.local_credentials.contains_key(&(VisibilityLevel::SelfScope as i32)));
    // Contact should NOT exist (since we started empty and only replenished Self)
    assert!(!local_creds.local_credentials.contains_key(&(VisibilityLevel::Contact as i32)));
}

#[rstest]
#[tokio::test]
#[serial]
#[allow(clippy::indexing_slicing)]
async fn test_device_id_change_does_not_trigger_refresh(mut test_env: TestEnv) {
    // This test verifies that changing the device_id does NOT trigger a metadata refresh
    // for existing credentials, but DOES affect newly generated credentials.

    // Get initial credentials
    let initial_creds =
        test_env.credentials_storage.get_local_credentials(&mock_default_provider()).unwrap();

    let initial_self_discovery = initial_creds
        .local_credentials
        .get(&(VisibilityLevel::SelfScope as i32))
        .unwrap()
        .converted_discovery_credentials[0]
        .clone();

    // Change device_id in preferences (simulating IdentityManager)
    let new_device_id = 1234567890u128.to_be_bytes();
    let _ = test_env.preferences.set_device_id(new_device_id);
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Get updated credentials - should be IDENTICAL to initial (no refresh)
    let updated_creds =
        test_env.credentials_storage.get_local_credentials(&mock_default_provider()).unwrap();

    let updated_self_discovery = updated_creds
        .local_credentials
        .get(&(VisibilityLevel::SelfScope as i32))
        .unwrap()
        .converted_discovery_credentials[0]
        .clone();

    // Self metadata should NOT change (optimization: we skip refresh on ID change)
    assert_eq!(
        initial_self_discovery.encrypted_metadata_bytes_v0,
        updated_self_discovery.encrypted_metadata_bytes_v0
    );

    // Wipe credentials before replenishing to mimic rotation and force strictly new credentials
    assert!(test_env
        .manager
        .wipeout_credentials(&mock_default_provider(), Some(VisibilityLevel::SelfScope))
        .is_ok());

    // Replenish credentials
    assert!(test_env
        .manager
        .replenish_credentials(&mock_default_provider(), Some(VisibilityLevel::SelfScope))
        .is_ok());
    let mut local_creds_changed = test_env.local_discovery_credentials_changed;
    let discovery_creds = local_creds_changed.recv().await.unwrap();
    test_env
        .manager
        .save_primed_credentials(&mock_default_provider(), discovery_creds.0 .1)
        .unwrap();

    // Get replenished credentials
    let replenished_creds =
        test_env.credentials_storage.get_local_credentials(&mock_default_provider()).unwrap();

    let replenished_self_discovery = replenished_creds
        .local_credentials
        .get(&(VisibilityLevel::SelfScope as i32))
        .unwrap()
        .converted_discovery_credentials[0]
        .clone();

    // Replenished metadata SHOULD be different (new ID used)
    assert_ne!(
        initial_self_discovery.encrypted_metadata_bytes_v0,
        replenished_self_discovery.encrypted_metadata_bytes_v0
    );
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_get_frequent_discovery_credential_book_initially_empty(test_env: TestEnv) {
    let book = test_env.manager.get_frequent_contact_discovery_credential_book().unwrap();
    assert_eq!(book.v0_iter().count(), 0);
    assert_eq!(book.v1_iter().count(), 0);
}

#[rstest]
#[tokio::test]
#[serial]
async fn test_get_on_demand_discovery_credential_book_returns_all_db_creds(test_env: TestEnv) {
    let book = test_env.manager.get_on_demand_contact_discovery_credential_book().unwrap();
    assert_eq!(book.v0_iter().count(), 2);
    assert_eq!(book.v1_iter().count(), 2);
}
