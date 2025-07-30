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

//! This is a metrics collector for the size of bincode-encoded messages, disguised as a test.
//!
//! Both network update messages and persistent storage contains the same data type so this is
//! applicable to both. However, storage always writes the full [`Document`], including elements
//! that are unchanged, whereas network updates will only send a [`DeltaDocument`], either generated
//! by the mutation operation, or by a call to [`Document::calculate_delta`].

use std::mem::size_of;

use crdt::{
    lww_map::LwwMapWrite, register::RegisterWrite, set::SetWrite, vector_data::VectorDataWrite,
};
use distributed_time::{vector_clock::VectorClock, DistributedClock};
use submerge::{
    testing::{SerializationTestDataStoreConfig, TestTimestampProvider},
    DataStoreConfig, SubmergeContainer, SubmergeMap, SubmergeRegister, SubmergeSet,
    SubmergeVectorData, UpdateContext,
};

#[test]
fn vector_clock_bincode_size() {
    // Opportunities for optimization:
    // - Turn on varint encoding
    //   https://docs.rs/bincode/1.3.3/bincode/config/trait.Options.html#method.with_varint_encoding.
    // - For longer node IDs that are not u16 (e.g. generated UUIDs), store them externally since
    //   the node ID tends to be repeated a lot. e.g.
    //       { "nodes": ["long-uuid-1", "long-uuid-2"], { ..., clock: [1, 2], ..., clock: [1, 0] } }
    //   instead of
    //       { { ..., clock: { "long-uuid-1": 1, "long-uuid-2": 2}, ..., clock: { "long-uuid-1": 1, "long-uuid-2": 0} } }
    let mut clock =
        VectorClock::<<SerializationTestDataStoreConfig as DataStoreConfig>::NodeId>::default();
    assert_eq!(8, bincode::serialize(&clock).unwrap().len());

    fn expected_size(num_nodes: usize) -> usize {
        // See https://github.com/bincode-org/bincode/blob/trunk/docs/spec.md#collections
        8 // length of the vector clock, encoded as u64
        + num_nodes * 13 // Two bytes each for the key
        + num_nodes * 4 // Four bytes each for the value
    }

    for i in 1_usize..=10 {
        clock = clock.increment(&format!("{i:0>5}")).unwrap();
        let serialized = bincode::serialize(&clock).unwrap();
        assert_eq!(
            expected_size(i),
            serialized.len(),
            "iteration {i}. Serialized={serialized:?}. clock={clock:?}"
        );
    }
}

#[test]
fn empty_bincode_size() {
    let doc = SubmergeContainer::<SerializationTestDataStoreConfig>::default();
    assert_eq!(9, bincode::serialize(&doc).unwrap().len());
}

#[test]
fn map_bincode_size() {
    fn create_map_with_size(size: usize) -> SubmergeMap<SerializationTestDataStoreConfig> {
        let ctx = UpdateContext::<SerializationTestDataStoreConfig> {
            node_id: "12345".to_string(),
            timestamp_provider: TestTimestampProvider,
            version: VectorClock::default(),
        };
        let mut map = SubmergeMap::<SerializationTestDataStoreConfig>::default();
        for i in 0..size {
            map.remove_value(&ctx, format!("{i:0>5}")).unwrap();
        }
        map
    }

    fn expected_size(key_size: usize, num_items: usize) -> usize {
        let node_id_size = 5;
        (key_size + node_id_size + 52) * num_items + 8
    }

    let (actual_size, expected_size): (Vec<_>, Vec<_>) = [10, 100, 1000, 1000]
        .into_iter()
        .map(|size| {
            let doc = create_map_with_size(size);
            (
                bincode::serialize(&doc).unwrap().len(),
                expected_size(5, size),
            )
        })
        .unzip();
    assert_eq!(actual_size, expected_size);
}

#[test]
fn register_bincode_size() {
    fn expected_size(data_size: usize) -> usize {
        data_size + 46
    }

    let ctx = UpdateContext::<SerializationTestDataStoreConfig> {
        node_id: 0.to_string(),
        timestamp_provider: TestTimestampProvider,
        version: VectorClock::default(),
    };
    let mut data = SubmergeRegister::<SerializationTestDataStoreConfig>::default();

    for i in [0, 10, 100, 1000] {
        data.set(&ctx, vec![0_u8; i]).unwrap();
        assert_eq!(expected_size(i), bincode::serialize(&data).unwrap().len());
    }
}

#[test]
fn set_bincode_size() {
    fn expected_size(data_size: usize, num_items: usize) -> usize {
        (data_size + 13) * num_items + 8
    }

    let mut data = SubmergeSet::<SerializationTestDataStoreConfig>::default();
    for i in [1_u32, 10, 100, 1000] {
        for v in (0..i).map(|i| i.to_be_bytes().to_vec()) {
            let _ = data.add(v).unwrap();
        }
        assert_eq!(
            expected_size(size_of::<u32>(), i as usize),
            bincode::serialize(&data).unwrap().len()
        );
    }

    let mut data = SubmergeSet::<SerializationTestDataStoreConfig>::default();
    for i in [1_u64, 10, 100, 1000] {
        for v in (0..i).map(|i| i.to_be_bytes().to_vec()) {
            let _ = data.add(v).unwrap();
        }
        assert_eq!(
            expected_size(size_of::<u64>(), usize::try_from(i).unwrap()),
            bincode::serialize(&data).unwrap().len()
        );
    }
}

#[test]
fn vector_data_bincode_size() {
    fn expected_size(data_size: usize, num_nodes: usize) -> usize {
        const KEY_SIZE: usize = 5;
        (KEY_SIZE + 8 // Key = Length prefix + key
            // Each VecEntry = {version: u32, data: Option<Vec<u8>>}
            + 4 // u32 version
            + 1 // Option has a 1-byte overhead
            + 8 // 8-byte length prefix of Vec<u8>
            + data_size
        ) * num_nodes
        // Map overhead = Length prefix = 8 bytes
        + 8
    }

    let (expected, actual): (Vec<_>, Vec<_>) = [0_usize, 1, 10, 100]
        .into_iter()
        .map(|x| {
            let mut data = SubmergeVectorData::<SerializationTestDataStoreConfig>::default();
            for i in 0..x {
                let _ = data.set(format!("{i:0>5}"), Some(vec![1_u8; 10])).unwrap();
            }
            (
                expected_size(10, x),
                bincode::serialize(&data).unwrap().len(),
            )
        })
        .collect();
    assert_eq!(expected, actual);

    // Changing value does not change the size
    let mut data = SubmergeVectorData::<SerializationTestDataStoreConfig>::default();
    for i in 0..100 {
        let _ = data.set(String::from("00000"), Some(vec![i; 10])).unwrap();
    }
    assert_eq!(
        expected_size(10, 1),
        bincode::serialize(&data).unwrap().len()
    );
}
