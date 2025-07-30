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

//! Internal crate for Submerge, containing the protobuf schema definition and corresponding traits.
//!
//! This is an internal implementation of Submerge. To enable proto serialization / deserialization,
//! use `submerge` with the `proto` feature.

use std::collections::BTreeMap;

use thiserror::Error;

/// Module containing types generated from the proto schema.
#[allow(clippy::unwrap_used, clippy::panic, clippy::cast_possible_truncation)]
#[cfg(feature = "cargo")]
pub mod protos {
    include!(concat!(env!("OUT_DIR"), "/proto/mod.rs"));
}

#[cfg(not(feature = "cargo"))]
pub use submerge_internal_proto_proto as protos;

/// A type that can be converted into a proto.
pub trait ToProto {
    /// The proto-generated type to be converted into.
    type Proto;

    /// Converts this type into a proto.
    ///
    /// # Arguments
    ///
    /// * `node_ids`: A mutable mapping from string node IDs to its index. This is used to compress
    ///   the payload since the node ID may be long and may appear multiple times in a document
    ///   (e.g. in each vector data and vector clock). The resulting list of `node_ids` will be
    ///   included in the top level Document or DeltaDocument proto.
    fn to_proto(&self, node_ids: &mut NodeMapping<String>) -> Self::Proto;
}

/// Error type when conversion from proto failed.
#[derive(Debug, Error)]
pub enum FromProtoError {
    /// The proto references a node ID index that is non-existent on the node_ids list given.
    ///
    /// This is an error in the input proto payload, and typically indicates a serialization error,
    /// such as using the wrong `NodeMapping` when serializing the `Document` or `DeltaDocument`.
    #[error("Referenced Node ID missing")]
    MissingNodeId,
    /// Numeric value overflowed.
    ///
    /// Protobufs only support 32-bit and 64-bit integers, while some values in the document are u8
    /// or u16. If the deserialization code encounters a numeric value that cannot fit in the target
    /// type, this error will be returned. This typically indicates an error in the serialization
    /// code.
    #[error("Numeric value overflowed")]
    NumericOverflow,
    /// Missing a required field.
    ///
    /// Proto 3 treats all non-primitive fields as optional, but in Submerge some fields are
    /// required to be successfully parsed into their corresponding Rust types.
    #[error("Required field missing")]
    MissingRequiredField,
    /// Length of the elements in the vector clock mismatched.
    ///
    /// A vector clock proto contains two repeated fields that are zipped together into a map, and
    /// therefore must have the same length.
    #[error("Mismatched vector clock")]
    MismatchedVectorClockLength,
}

/// A trait indicating it can be constructed from a proto.
pub trait FromProto: Sized {
    /// The proto-generated type to parse into the corresponding Rust type.
    type Proto;

    /// Construct an instance from the given proto.
    fn from_proto(proto: &Self::Proto, node_ids: &[String]) -> Result<Self, FromProtoError>;
}

/// A mapping between node IDs and its corresponding index.
///
/// This is used to compress the serialized data as the node IDs are often long (e.g. 36-character
/// UUID) and appears in a document multiple times. This structure collects all of the node IDs used
/// during serialization and allows them to be replaced with the numeric index.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct NodeMapping<N: Ord>(BTreeMap<N, u32>);

impl<N: Ord + Clone> NodeMapping<N> {
    /// Gets the index for the given `node_id`.
    ///
    /// If the `node_id` doesn't already exist, it will be added to the map.
    pub fn get_index(&mut self, node_id: &N) -> u32 {
        let len = self.0.len();
        *self
            .0
            .entry(node_id.clone())
            .or_insert(len.try_into().expect("Too many node IDs"))
    }

    /// Converts this node mapping into a vector.
    ///
    /// The resulting vector will contain all the node IDs sorted by its value. Since the indices
    /// are allocated in a contiguous range, the vec index can be used to look up the corresponding
    /// node ID.
    pub fn into_vec(self) -> Vec<N> {
        let reverse_map = self
            .0
            .into_iter()
            .map(|(k, v)| (v, k))
            .collect::<BTreeMap<_, _>>();
        debug_assert_eq!(
            reverse_map.keys().map(|v| *v as usize).collect::<Vec<_>>(),
            (0..reverse_map.len()).collect::<Vec<_>>()
        );
        reverse_map.values().cloned().collect()
    }
}
