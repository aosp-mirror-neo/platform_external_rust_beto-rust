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

//! Defines the proto serialization of submerge documents.

use distributed_time::vector_clock::VectorClock;
use protobuf::Message;
use submerge_internal_proto::{FromProto, FromProtoError, NodeMapping, ToProto};
use thiserror::Error;

use crate::{DataStoreConfig, DeltaDocument, Document, SubmergeContainer};

/// Error when serializing or deserializing a proto.
#[derive(Debug, Error)]
pub enum ProtoSerdeError {
    /// Error deserializing the proto into its corresponding Rust type.
    #[error(transparent)]
    FromProtoError(#[from] FromProtoError),
    /// Error in serializing or deserializing bytes into the proto-generated type.
    #[error(transparent)]
    ProtobufError(#[from] protobuf::Error),
}

impl<Conf> Document<Conf>
where
    Conf: DataStoreConfig<NodeId = String, ValueType = Vec<u8>>,
{
    fn to_proto(&self) -> submerge_internal_proto::protos::submerge::Document {
        let mut node_ids = NodeMapping::default();
        let root = Some(self.root.to_proto(&mut node_ids));
        let version = self.version.to_proto(&mut node_ids);
        submerge_internal_proto::protos::submerge::Document {
            node_ids: node_ids.into_vec(),
            version: Some(version).into(),
            root: root.into(),
            ..Default::default()
        }
    }

    /// Convert the Document to a serialized proto that can be persisted.
    pub fn to_proto_bytes(&self) -> Result<Vec<u8>, ProtoSerdeError> {
        Ok(self.to_proto().write_to_bytes()?)
    }

    fn from_proto(
        proto: &submerge_internal_proto::protos::submerge::Document,
    ) -> Result<Self, FromProtoError> {
        Ok(Document {
            root: SubmergeContainer::<Conf>::from_proto(&proto.root, &proto.node_ids)?,
            version: VectorClock::from_proto(&proto.version, &proto.node_ids)?,
        })
    }

    /// Construct a Document from the given serialized proto.
    pub fn from_proto_bytes(proto_bytes: &[u8]) -> Result<Self, ProtoSerdeError> {
        Ok(Self::from_proto(
            &submerge_internal_proto::protos::submerge::Document::parse_from_bytes(proto_bytes)?,
        )?)
    }
}

impl<Conf> DeltaDocument<Conf>
where
    Conf: DataStoreConfig<NodeId = String, ValueType = Vec<u8>>,
{
    fn to_proto(&self) -> submerge_internal_proto::protos::submerge::DeltaDocument {
        let mut node_ids = NodeMapping::default();
        let root = Some(self.root.to_proto(&mut node_ids));
        let version = self.version.to_proto(&mut node_ids);
        let base_version = self.base_version.to_proto(&mut node_ids);
        submerge_internal_proto::protos::submerge::DeltaDocument {
            node_ids: node_ids.into_vec(),
            version: Some(version).into(),
            base_version: Some(base_version).into(),
            root: root.into(),
            ..Default::default()
        }
    }

    /// Convert the DeltaDocument to a serialized proto that can be sent over the wire.
    pub fn to_proto_bytes(&self) -> Result<Vec<u8>, ProtoSerdeError> {
        Ok(self.to_proto().write_to_bytes()?)
    }

    fn from_proto(
        proto: &submerge_internal_proto::protos::submerge::DeltaDocument,
    ) -> Result<Self, FromProtoError> {
        Ok(Self {
            root: SubmergeContainer::<Conf>::from_proto(&proto.root, &proto.node_ids)?,
            base_version: VectorClock::from_proto(&proto.base_version, &proto.node_ids)?,
            version: VectorClock::from_proto(&proto.version, &proto.node_ids)?,
        })
    }

    /// Construct a DeltaDocument from the given serialized proto.
    pub fn from_proto_bytes(proto_bytes: &[u8]) -> Result<Self, ProtoSerdeError> {
        Ok(Self::from_proto(
            &submerge_internal_proto::protos::submerge::DeltaDocument::parse_from_bytes(
                proto_bytes,
            )?,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use crate::{testing::SerializationTestDataStoreConfig, DeltaDocument, Document};

    #[derive_fuzztest::proptest]
    fn document_round_trip(doc: Document<SerializationTestDataStoreConfig>) {
        assert_eq!(
            Document::<SerializationTestDataStoreConfig>::from_proto(&doc.to_proto()).unwrap(),
            doc
        );
    }

    #[derive_fuzztest::proptest]
    fn delta_document_round_trip(doc: DeltaDocument<SerializationTestDataStoreConfig>) {
        assert_eq!(
            DeltaDocument::<SerializationTestDataStoreConfig>::from_proto(&doc.to_proto()).unwrap(),
            doc
        );
    }
}
