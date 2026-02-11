#![allow(clippy::expect_used)]
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

use crate::identity_storage::credentials_database::{
    create_credentials_database, create_database_storage, CredentialsDatabase,
};
use credential_proto::proto::credential_book::{
    ConsumedSalts, ConsumedSaltsBook, LocalCredentialBook, RemoteCredentialBook,
};
use identity_common::definitions::ProviderName;
use serial_test::serial;
use std::sync::Arc;

fn create_db_with_data() -> Arc<dyn CredentialsDatabase> {
    let creds_db = create_credentials_database(
        create_database_storage().expect("Failed to get local app data dir"),
    );
    let provider1 = ProviderName("provider1".to_string());
    let provider2 = ProviderName("provider2".to_string());

    creds_db
        .insert_local_credentials(&provider1, LocalCredentialBook::default())
        .expect("Failed to insert local credentials");
    creds_db
        .insert_remote_credentials(&provider1, RemoteCredentialBook::default())
        .expect("Failed to insert remote credentials");
    creds_db
        .insert_consumed_salts(&provider1, ConsumedSaltsBook::default())
        .expect("Failed to insert consumed salts");
    creds_db
        .insert_local_credentials(&provider2, LocalCredentialBook::default())
        .expect("Failed to insert local credentials");
    creds_db
        .insert_remote_credentials(&provider2, RemoteCredentialBook::default())
        .expect("Failed to insert remote credentials");
    creds_db
        .insert_consumed_salts(&provider2, ConsumedSaltsBook::default())
        .expect("Failed to insert consumed salts");
    creds_db
}

#[test]
#[serial]
fn test_delete_all_credentials() {
    let creds_db = create_db_with_data();
    let provider1 = ProviderName("provider1".to_string());
    let provider2 = ProviderName("provider2".to_string());

    creds_db.delete_all_credentials().expect("Failed to delete all credentials");

    assert!(creds_db.get_local_credentials(&provider1).is_err());
    assert!(creds_db.get_remote_credentials(&provider1).is_err());
    assert!(creds_db.get_consumed_salts(&provider1).is_err());
    assert!(creds_db.get_local_credentials(&provider2).is_err());
    assert!(creds_db.get_remote_credentials(&provider2).is_err());
    assert!(creds_db.get_consumed_salts(&provider2).is_err());
}

#[test]
#[serial]
fn test_delete_credentials_for_provider() {
    let creds_db = create_db_with_data();
    let provider1 = ProviderName("provider1".to_string());
    let provider2 = ProviderName("provider2".to_string());

    creds_db
        .delete_credentials_for_provider(&provider1)
        .expect("Failed to delete credentials for provider");

    assert!(creds_db.get_local_credentials(&provider1).is_err());
    assert!(creds_db.get_remote_credentials(&provider1).is_err());
    assert!(creds_db.get_consumed_salts(&provider1).is_err());
    assert!(creds_db.get_local_credentials(&provider2).is_ok());
    assert!(creds_db.get_remote_credentials(&provider2).is_ok());
    assert!(creds_db.get_consumed_salts(&provider2).is_ok());
}

#[test]
#[serial]
fn test_insert_and_get_consumed_salts() {
    let creds_db = create_credentials_database(
        create_database_storage().expect("Failed to get local app data dir"),
    );
    let provider = ProviderName("provider".to_string());
    let mut consumed_salts_book = ConsumedSaltsBook::default();
    let mut consumed_salts = ConsumedSalts::default();
    consumed_salts.salt.push(vec![1, 2, 3]);
    let _ = consumed_salts_book.consumed_salts_map.insert(123, consumed_salts);

    creds_db
        .insert_consumed_salts(&provider, consumed_salts_book.clone())
        .expect("Failed to insert consumed salts");

    let retrieved_salts =
        creds_db.get_consumed_salts(&provider).expect("Failed to get consumed salts");

    assert_eq!(consumed_salts_book, retrieved_salts);
}

#[test]
#[serial]
fn test_insert_and_get_local_credentials() {
    let creds_db = create_credentials_database(
        create_database_storage().expect("Failed to get local app data dir"),
    );

    let provider = ProviderName("provider".to_string());

    let local_credential_book = LocalCredentialBook::default();

    creds_db
        .insert_local_credentials(&provider, local_credential_book.clone())
        .expect("Failed to insert local credentials");

    let retrieved_creds =
        creds_db.get_local_credentials(&provider).expect("Failed to get local credentials");

    assert_eq!(local_credential_book, retrieved_creds);
}

#[test]
#[serial]
fn test_insert_and_get_remote_credentials() {
    let creds_db = create_credentials_database(
        create_database_storage().expect("Failed to get local app data dir"),
    );

    let provider = ProviderName("provider".to_string());

    let remote_credential_book = RemoteCredentialBook::default();

    creds_db
        .insert_remote_credentials(&provider, remote_credential_book.clone())
        .expect("Failed to insert remote credentials");

    let retrieved_creds =
        creds_db.get_remote_credentials(&provider).expect("Failed to get remote credentials");

    assert_eq!(remote_credential_book, retrieved_creds);
}
