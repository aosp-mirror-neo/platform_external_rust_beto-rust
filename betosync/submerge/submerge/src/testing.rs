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

//! Utilities and fake implementations for testing.

use distributed_time::WallTimestampProvider;

use crate::{DataStoreConfig, DeltaDocument, Document, NetworkInterface, StorageInterface};

/// A [`DataStoreConfig`] for testing serialization.
pub struct SerializationTestDataStoreConfig;

impl DataStoreConfig for SerializationTestDataStoreConfig {
    type NodeId = String;
    type ValueType = Vec<u8>;
    type Network = TestNetworkInterface;
    type Storage = TestStorageInterface;
    type TimestampProvider = TestTimestampProvider;
}

/// A network interface for testing that panics when a new update a received.
#[derive(Debug, Clone)]
pub struct TestNetworkInterface;

impl NetworkInterface<SerializationTestDataStoreConfig> for TestNetworkInterface {
    fn on_new_update(
        &mut self,
        _doc_id: &str,
        _document: DeltaDocument<SerializationTestDataStoreConfig>,
    ) {
        unimplemented!()
    }
}

/// A storage interface for testing that panics when an update is received
#[derive(Debug, Clone)]
pub struct TestStorageInterface;

impl StorageInterface<SerializationTestDataStoreConfig> for TestStorageInterface {
    type Error = ();

    fn on_new_update(
        &mut self,
        _doc_id: &str,
        _document: &Document<SerializationTestDataStoreConfig>,
    ) -> Result<(), Self::Error> {
        unimplemented!()
    }

    fn read_from_storage(
        &self,
        _doc_id: &str,
    ) -> Result<Option<Document<SerializationTestDataStoreConfig>>, Self::Error> {
        unimplemented!()
    }
}

/// A test timestamp provider that returns a fixed timestamp.
#[derive(Debug, Clone)]
pub struct TestTimestampProvider;

impl WallTimestampProvider for TestTimestampProvider {
    fn now(&self) -> u64 {
        // An arbitrary, but somewhat realistic timestamp, so that the sizes are still accurate even
        // if integers are encoded with, say, variable-length-quantity.
        1720000000000
    }
}
