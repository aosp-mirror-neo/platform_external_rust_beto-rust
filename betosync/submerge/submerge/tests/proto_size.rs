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

//! This is a metrics collector for the size of proto-encoded messages, disguised as a test.
//!
//! Both network update messages and persistent storage contains the same data type so this is
//! applicable to both. However, storage always writes the full [`Document`], including elements
//! that are unchanged, whereas network updates will only send a [`DeltaDocument`], either generated
//! by the mutation operation, or by a call to [`Document::calculate_delta`].
//!
//! [Protoscope](https://github.com/protocolbuffers/protoscope) can be used to inspect binary
//! serialized protobufs to see how they are encoded.
//!
//!     e.g. `xxd -r -ps <<< "0a0e120a000000000000000000001801" | protoscope`
//!
//! Also see https://protobuf.dev/programming-guides/encoding/#packed for how to interpret the
//! output. As an approximation, `<number>: { ... }` is a TLV that incurs 2 bytes of overhead, and
//! `<number>: <another number>` incurs 2 bytes, assuming the numeric value is less than 128.

use std::mem::size_of;

use crdt::{
    lww_map::LwwMapWrite, register::RegisterWrite, set::SetWrite, vector_data::VectorDataWrite,
};
use distributed_time::{vector_clock::VectorClock, DistributedClock};
use protobuf::Message;
use submerge::{
    testing::{SerializationTestDataStoreConfig, TestTimestampProvider},
    DataStoreConfig, SubmergeContainer, SubmergeMap, SubmergeRegister, SubmergeSet,
    SubmergeVectorData, UpdateContext,
};
use submerge_internal_proto::{NodeMapping, ToProto};

const PRINT_PROTOSCOPE_COMMANDS: bool = true;

#[test]
fn vector_clock_proto_size() {
    let mut node_mapping = NodeMapping::default();
    let mut clock =
        VectorClock::<<SerializationTestDataStoreConfig as DataStoreConfig>::NodeId>::default();
    assert_eq!(
        0,
        clock
            .to_proto(&mut node_mapping)
            .write_to_bytes()
            .unwrap()
            .len()
    );

    fn expected_size(num_nodes: usize) -> usize {
        4 // 4 bytes overhead (2 fields with type and length)
        + num_nodes * 2 // 2 bytes per each value (assuming varint value < 128)
    }

    for i in 1_usize..=10 {
        clock = clock.increment(&i.to_string()).unwrap();
        let serialized = clock.to_proto(&mut node_mapping).write_to_bytes().unwrap();
        assert_eq!(
            expected_size(i),
            serialized.len(),
            "iteration {i}. Serialized={serialized:?}. clock={clock:?}"
        );
    }
}

#[test]
fn empty_proto_size() {
    let mut node_mapping = NodeMapping::default();
    let doc = SubmergeContainer::<SerializationTestDataStoreConfig>::default();
    assert_eq!(
        2,
        doc.to_proto(&mut node_mapping)
            .write_to_bytes()
            .unwrap()
            .len()
    );
}

#[test]
fn map_proto_size() {
    fn create_map_with_size(size: usize) -> SubmergeMap<SerializationTestDataStoreConfig> {
        let ctx = UpdateContext::<SerializationTestDataStoreConfig> {
            node_id: "0".to_string(),
            timestamp_provider: TestTimestampProvider,
            version: VectorClock::default(),
        };
        let mut map = SubmergeMap::<SerializationTestDataStoreConfig>::default();
        for i in 0..size {
            map.remove_value(&ctx, format!("{i:05}")).unwrap();
        }
        map
    }

    fn expected_size(key_size: usize, num_items: usize) -> usize {
        (key_size + 25) * num_items
    }

    let mut node_mapping = NodeMapping::default();
    let (actual_size, expected_size): (Vec<_>, Vec<_>) = [10, 100, 1000]
        .into_iter()
        .map(|size| {
            let doc = create_map_with_size(size);
            let bytes = doc.to_proto(&mut node_mapping).write_to_bytes().unwrap();
            if PRINT_PROTOSCOPE_COMMANDS {
                eprintln!(
                    "Map with {size} null elements: {}",
                    protoscope_command(&bytes)
                );
            }
            (bytes.len(), expected_size(5, size))
        })
        .unzip();
    assert_eq!(expected_size, actual_size);
}

#[test]
fn register_proto_size() {
    fn expected_size(data_size: usize) -> usize {
        match data_size {
            0 => 21,
            1..=127 => data_size + 23,
            128.. => data_size + 25,
        }
    }

    let mut node_mapping = NodeMapping::default();
    let ctx = UpdateContext::<SerializationTestDataStoreConfig> {
        node_id: "0".to_string(),
        timestamp_provider: TestTimestampProvider,
        version: VectorClock::default(),
    };
    let mut data = SubmergeRegister::<SerializationTestDataStoreConfig>::default();

    for i in [0, 10, 100, 1000] {
        data.set(&ctx, vec![0_u8; i]).unwrap();
        let bytes = data.to_proto(&mut node_mapping).write_to_bytes().unwrap();
        if PRINT_PROTOSCOPE_COMMANDS {
            eprintln!("Register with {i} elements: {}", protoscope_command(&bytes));
        }
        assert_eq!(expected_size(i), bytes.len());
    }
}

#[test]
fn set_proto_size() {
    fn expected_size(data_size: usize, num_items: usize) -> usize {
        (data_size + 6) * num_items
    }

    let mut node_mapping = NodeMapping::default();
    let mut data = SubmergeSet::<SerializationTestDataStoreConfig>::default();
    let (expected, actual): (Vec<_>, Vec<_>) = [1_u32, 10, 100, 1000]
        .iter()
        .map(|i| {
            for v in (0..*i).map(|i| i.to_be_bytes().to_vec()) {
                let _ = data.add(v).unwrap();
            }
            let bytes = data.to_proto(&mut node_mapping).write_to_bytes().unwrap();
            if PRINT_PROTOSCOPE_COMMANDS {
                eprintln!("Set with {i} elements: {}", protoscope_command(&bytes));
            }
            (expected_size(size_of::<u32>(), *i as usize), bytes.len())
        })
        .unzip();
    assert_eq!(expected, actual);

    let mut data = SubmergeSet::<SerializationTestDataStoreConfig>::default();
    for i in [1_u64, 10, 100, 1000] {
        for v in (0..i).map(|i| i.to_be_bytes().to_vec()) {
            let _ = data.add(v).unwrap();
        }
        assert_eq!(
            expected_size(size_of::<u64>(), usize::try_from(i).unwrap()),
            data.to_proto(&mut node_mapping)
                .write_to_bytes()
                .unwrap()
                .len()
        );
    }
}

#[test]
fn vector_data_proto_size() {
    fn expected_size(data_size: usize, num_nodes: usize) -> usize {
        (data_size + 8) * num_nodes // 8 bytes of overhead per node
            - 2 // For the first node, the node_id is omitted because the default is 0.
    }

    let mut node_mapping = NodeMapping::default();
    let mut data = SubmergeVectorData::<SerializationTestDataStoreConfig>::default();

    let (expected, actual): (Vec<_>, Vec<_>) = [1_usize, 10, 100]
        .iter()
        .map(|x| {
            for i in 0..*x {
                let _ = data.set(i.to_string(), Some(vec![0_u8; 10])).unwrap();
            }
            let bytes = data.to_proto(&mut node_mapping).write_to_bytes().unwrap();
            if PRINT_PROTOSCOPE_COMMANDS {
                eprintln!("VectorData with {x} nodes: {}", protoscope_command(&bytes));
            }
            (expected_size(10, *x), bytes.len())
        })
        .unzip();
    assert_eq!(expected, actual);

    // Changing value does not change the size
    let _ = data.set(0.to_string(), Some(vec![1_u8; 10])).unwrap();
    assert_eq!(
        expected_size(10, 100),
        data.to_proto(&mut node_mapping)
            .write_to_bytes()
            .unwrap()
            .len()
    );
}

/// Print the protoscope command to inspect the given serialized proto.
///
/// See https://github.com/protocolbuffers/protoscope.
fn protoscope_command(bytes: &[u8]) -> String {
    format!("xxd -r -ps <<< \"{}\" | protoscope", hex::encode(bytes))
}
