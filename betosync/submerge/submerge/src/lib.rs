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

//! submerge — data store designed for syncing
//!
//! submerge is an always-available, eventually consistent data store designed for use to
//! synchronize data across multiple devices.
//!
//! # Data model
//!
//! The data model for submerge consists of four types:
//! 1. `Map` — the map is the main structural type in submerge's data model. The key for the map is
//!    a string, and the value for the map is recursively another CRDT type.
//! 2. `Set` — a CRDT set (_causal-length set_) that can contain multiple unordered values.
//! 3. `Register` — a CRDT register that can contain a single atomically updated value.
//! 4. `VectorData` — a vector with an entry for each node in the system, where entries can only be
//!    written to by its owner.
//!
//! The value type is currently defined as raw bytes, but this can be changed in the future to allow
//! dynamic typing.
//!
//! # Interfaces
//! See [`NetworkInterface`] and [`StorageInterface`].

use std::{
    borrow::BorrowMut, cmp::Ordering, collections::hash_map::RandomState, fmt::Debug, hash::Hash,
};

use arbitrary::Arbitrary;
use crdt::{
    delta::DeltaMut,
    lww_crdt_container::LwwCrdtContainer,
    lww_map::LwwMap,
    register::Register,
    set::Set,
    typed_crdt::TypedCrdt,
    vector_data::{VectorData, VersionOverflow},
    CrdtState,
};
use derive_where::derive_where;
use distributed_time::{
    hybrid_logical_clock::HybridLogicalTimestamp, vector_clock::VectorClock, DistributedClock,
    TimestampOverflow, WallTimestampProvider,
};
use log::{debug, info};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(feature = "proto")]
pub mod proto;

#[cfg(any(test, feature = "checker"))]
pub mod testing;

/// The ID of a document.
pub type DocId = String;

type ValueType<Conf> = <Conf as DataStoreConfig>::ValueType;
type NodeId<Conf> = <Conf as DataStoreConfig>::NodeId;

/// Type alias for the map type for the given [`DataStoreConfig`].
pub type SubmergeMap<Conf> = LwwMap<
    String,
    TypedCrdt<ValueType<Conf>, NodeId<Conf>>,
    HybridLogicalTimestamp<NodeId<Conf>>,
    NodeId<Conf>,
>;
/// Type alias for the register type for the given [`DataStoreConfig`].
pub type SubmergeRegister<Conf> = Register<ValueType<Conf>, VectorClock<NodeId<Conf>>>;
/// Type alias for the set type for the given [`DataStoreConfig`].
pub type SubmergeSet<Conf> = Set<ValueType<Conf>, RandomState>;
/// Type alias for the vector data type for the given [`DataStoreConfig`].
pub type SubmergeVectorData<Conf> = VectorData<NodeId<Conf>, ValueType<Conf>>;

/// Type alias for the container type for the given [`DataStoreConfig`].
pub type SubmergeContainer<Conf> = LwwCrdtContainer<
    TypedCrdt<ValueType<Conf>, NodeId<Conf>>,
    HybridLogicalTimestamp<NodeId<Conf>>,
    NodeId<Conf>,
>;

/// A submerge document that implements the data model as described in the module documentation.
///
/// This document is a CRDT and guarantees eventual consistency. Instances of document should be
/// created by a [`DataStore`] via [`DataStore::open_or_create_document`].
#[derive(Serialize, Deserialize)]
#[derive_where(Default, Debug, Clone, PartialEq, Eq)]
#[serde(bound(
    serialize = "Conf::NodeId: Serialize, Conf::ValueType: Serialize",
    deserialize = "Conf::NodeId: for<'a> Deserialize<'a>, Conf::ValueType: for<'a> Deserialize<'a>"
))]
pub struct Document<Conf: DataStoreConfig> {
    /// The root container stored in this document.
    pub root: SubmergeContainer<Conf>,
    /// The version of this document. This is a vector clock that is incremented every time a
    /// transaction is committed, used to ensure causal consistency is not violated by incoming
    /// update messages.
    pub version: VectorClock<Conf::NodeId>,
}

// Manual implementation of Arbitrary to specify the correct trait bounds.
impl<'a, Conf: DataStoreConfig> Arbitrary<'a> for Document<Conf>
where
    Conf::NodeId: Arbitrary<'a>,
    Conf::ValueType: Arbitrary<'a>,
{
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self {
            root: u.arbitrary()?,
            version: u.arbitrary()?,
        })
    }
}

impl<Conf: DataStoreConfig> Document<Conf> {
    /// Calculates the delta needed to update from the given `base_version`.
    ///
    /// The `base_version` must be a version vector from a replica of the same document. The
    /// returned document can be applied using [`DeltaDocument::into_document`] by supplying a base
    /// that is at least as new as `base_version`.
    pub fn calculate_delta(&self, base_version: VectorClock<Conf::NodeId>) -> DeltaDocument<Conf> {
        if self.version <= base_version {
            // Return an empty delta
            return DeltaDocument {
                root: SubmergeContainer::<Conf>::default(),
                base_version,
                version: self.version.clone(),
            };
        }
        DeltaDocument {
            root: self.root.calculate_delta(&base_version),
            base_version,
            version: self.version.clone(),
        }
    }

    /// Transform this document into a full delta document.
    ///
    /// This "full" delta document is going to contain all data in this document, thus allowing it
    /// to be applied from any version.
    pub fn into_full_delta(self) -> DeltaDocument<Conf> {
        DeltaDocument {
            root: self.root,
            base_version: VectorClock::default(),
            version: self.version,
        }
    }
}

/// A delta document that holds a delta update for a document.
///
/// This delta contains information about the changes between [`base_version`][Self::base_version]
/// and [`version`][Self::version], and can be merged with documents as least as new as
/// `base_version` without violating causal consistency.
///
/// This is serialized and sent over the wire when an update is made or requested by a remote peer.
#[derive(Serialize, Deserialize)]
#[derive_where(Clone, PartialEq, Eq)]
#[derive_where(Debug)]
#[serde(bound(
    serialize = "Conf::NodeId: Serialize, Conf::ValueType: Serialize",
    deserialize = "Conf::NodeId: for<'a> Deserialize<'a>, Conf::ValueType: for<'a> Deserialize<'a>"
))]
pub struct DeltaDocument<Conf: DataStoreConfig> {
    /// The root of the delta component of the document.
    pub root: SubmergeContainer<Conf>,
    /// The base version at which this delta starts. This represents the version of the base
    /// component at which this delta is being constructed, meaning that to recreate the resulting
    /// document, the base document must be at least as new as this version.
    pub base_version: VectorClock<Conf::NodeId>,
    /// The version at which the delta document is at.
    pub version: VectorClock<Conf::NodeId>,
}

impl<'a, Conf: DataStoreConfig> Arbitrary<'a> for DeltaDocument<Conf>
where
    Conf::NodeId: Arbitrary<'a>,
    Conf::ValueType: Arbitrary<'a>,
{
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self {
            root: u.arbitrary()?,
            base_version: u.arbitrary()?,
            version: u.arbitrary()?,
        })
    }
}

/// Error returned when a delta is not applicable to the given base.
#[derive(Debug, Clone, Error)]
#[error("Delta is not applicable to the given base")]
pub struct DeltaNotApplicable;

impl<Conf: DataStoreConfig> DeltaDocument<Conf> {
    /// Merge this delta document into the given `base`.
    ///
    /// The given `base` should be generated with [`Document::calculate_delta`] or
    /// [`DeltaOwned::into_delta`][crdt::delta::DeltaOwned::into_delta], and must be at least as new
    /// as the [`base_version`][Self::base_version], or `Err(DeltaNotApplicable)` will be returned.
    ///
    /// The resulting document will contain all changes from both this delta and the `base`, merged
    /// together according to the CRDT definitions.
    pub fn into_document(
        self,
        base: &Document<Conf>,
    ) -> Result<Document<Conf>, DeltaNotApplicable> {
        if let None | Some(Ordering::Less) = base.version.partial_cmp(&self.base_version) {
            Err(DeltaNotApplicable)
        } else {
            Ok(Document {
                root: CrdtState::merge(&self.root, &base.root),
                version: VectorClock::least_upper_bound(&self.version, &base.version),
            })
        }
    }
}

/// Error when trying to commit a transaction to a document.
#[derive(Error)]
#[derive_where(Debug; StorageError<Conf>)]
pub enum TransactionError<Conf: DataStoreConfig> {
    /// Cannot perform the transaction because the timestamp overflowed.
    #[error("Cannot perform the transaction because the timestamp overflowed.")]
    TimestampOverflow,
    /// Cannot perform the transaction because the version number overflowed.
    #[error("Cannot perform the transaction because the version number overflowed.")]
    VersionOverflow,
    /// The storage interface failed to persist the resulting data.
    #[error(transparent)]
    StorageError(StorageError<Conf>),
}

impl<Conf: DataStoreConfig> From<TimestampOverflow> for TransactionError<Conf> {
    fn from(_: TimestampOverflow) -> Self {
        TransactionError::TimestampOverflow
    }
}

impl<Conf: DataStoreConfig> From<VersionOverflow> for TransactionError<Conf> {
    fn from(_: VersionOverflow) -> Self {
        TransactionError::VersionOverflow
    }
}

/// The network interface for a data store.
///
/// When the data store is modified, updated documents will be generated and passed to
/// `on_new_update`. The network interface should make a best-effort to send the update messages to
/// other nodes on the network, potentially retrying if certain nodes are unreachable.
///
/// It is safe for the network interface to send the same messages multiple times, or out of order.
/// Eventual consistency of the documents are guaranteed as long as the update messages are
/// eventually delivered.
///
/// The data store will not wait for the network message to be sent or delivered. If the network
/// layer wishes to get the latest state to send over the network, it can do so by calling
/// [`DataStore::snapshot`].
pub trait NetworkInterface<Conf: DataStoreConfig> {
    /// Called when a new update to the data store should be sent to to other nodes over the
    /// network.
    ///
    /// Failed updates are ignored and reconciled when the device reconnects. Alternatively, the
    /// network implementation can get the latest snapshot from the datastore and sync that.
    fn on_new_update(&mut self, doc_id: &str, document: DeltaDocument<Conf>);
}

/// The storage interface for a data store.
///
/// When the data store is modified, or when new update messages are received from the network,
/// `on_new_update` will be called to request the storage layer to persist the latest state. The
/// storage API is fallible, and failure to persist the data will result in a corresponding
/// [`TransactionError`].
pub trait StorageInterface<Conf: DataStoreConfig> {
    /// The error type for this storage.
    type Error: Debug;

    /// Called when a new update to the data store should be persisted to storage.
    fn on_new_update(&mut self, doc_id: &str, document: &Document<Conf>)
        -> Result<(), Self::Error>;

    /// Read a serialized document from storage.
    ///
    /// This function returns `None` if there are no documents with the given `doc_id`.
    fn read_from_storage(&self, doc_id: &str) -> Result<Option<Document<Conf>>, Self::Error>;
}

/// The update context for a given submerge data store.
#[derive_where(Debug, Clone)]
pub struct UpdateContext<Conf: DataStoreConfig> {
    /// The local node ID representing the device this code is running on. This ID must be unique
    /// among the group that participates in syncing this [`DataStore`].
    pub node_id: Conf::NodeId,
    /// The timestamp provider that gives the wall-clock time. See [`WallTimestampProvider`].
    pub timestamp_provider: Conf::TimestampProvider,
    /// The version of the document.
    ///
    /// The CRDT implementations assume that this context is monotonically increasing, meaning that
    /// the version may be copied into the CRDT tree, and any later operations' context must return
    /// a version newer than or equal to the copied version value.
    ///
    /// See [`crdt::UpdateContext::version`].
    pub version: VectorClock<Conf::NodeId>,
}

impl<Conf: DataStoreConfig> crdt::UpdateContext<Conf::NodeId> for UpdateContext<Conf> {
    type WallClock = Conf::TimestampProvider;

    fn updater(&self) -> &Conf::NodeId {
        &self.node_id
    }

    fn timestamp_provider(&self) -> &Self::WallClock {
        &self.timestamp_provider
    }

    fn version(&self) -> &VectorClock<Conf::NodeId> {
        &self.version
    }
}

/// A document with its metadata.
///
/// This handle holds the document together with the node-specific metadata needed to update it,
/// such as the local node ID and the wall-clock timestamp provider.
#[derive_where(Debug)]
pub struct DocumentHandle<Conf: DataStoreConfig> {
    id: DocId,
    doc: Document<Conf>,
    metadata: NodeMetadata<Conf>,
}

/// Type-level configuration for the [`DataStore`].
///
/// Implementations do not need to be instantiable, and can be implemented by unit structs or even
/// zero-variant enums.
pub trait DataStoreConfig: Sized {
    /// The type representing each node in this data store. This node ID is assigned when the
    /// [`DataStore`] is constructed, must be unique among all of the syncing devices, and must not
    /// be reused.
    type NodeId: Ord + Clone + Debug + 'static;
    /// The value type shared between all types in the data model. Values in `Set`, `Register`, and
    /// `VectorData` all store instances of this type.
    type ValueType: Eq + Hash + Clone + Debug + 'static;
    /// The network interface used to send update messages to other peer nodes.
    ///
    /// See [`NetworkInterface`].
    type Network: NetworkInterface<Self> + Debug + Clone;
    /// The storage interface used to persist new versions of the document, and read persisted
    /// documents back from disk.
    ///
    /// See [`StorageInterface`].
    type Storage: StorageInterface<Self> + Debug + Clone;
    /// The timestamp provider used to provide wall clock timestamps.
    ///
    /// See [`WallTimestampProvider`] for the format and requirements of the timestamps.
    type TimestampProvider: WallTimestampProvider + Debug + Clone;
}

/// A transaction for a given document.
///
/// This transaction temporarily accumulates mutations until it is [`commit`][Self::commit] is
/// called.
#[derive_where(Debug; Handle)]
pub struct DocumentTransaction<Handle, Conf>
where
    Handle: BorrowMut<DocumentHandle<Conf>>,
    Conf: DataStoreConfig,
{
    /// The document handle this transaction is for.
    handle: Handle,
    /// The delta component of this transaction (containing changes made during this transaction).
    delta: SubmergeContainer<Conf>,
    /// The update context for this document transaction.
    update_context: UpdateContext<Conf>,
}

impl<Handle, Conf> DocumentTransaction<Handle, Conf>
where
    Handle: BorrowMut<DocumentHandle<Conf>>,
    Conf: DataStoreConfig,
{
    /// Create a transaction for this document.
    ///
    /// The function for both are similar, the main difference is that [`DocumentTransaction`] uses
    /// the explicit [`commit`][DocumentTransaction::commit] method, whereas
    /// [`DocumentHandle::modify`] automatically commits when the closure exits.
    pub fn new(handle: Handle) -> Self {
        let handle_borrowed = handle.borrow();
        let update_context = UpdateContext {
            node_id: handle_borrowed.metadata.node_id.clone(),
            timestamp_provider: handle_borrowed.metadata.timestamp_provider.clone(),
            version: handle_borrowed.doc.version.clone(),
        };
        Self {
            handle,
            delta: Default::default(),
            update_context,
        }
    }

    /// Get a mutable reference to the root document and the update context.
    pub fn get_context_and_root_mut(
        &mut self,
    ) -> (&UpdateContext<Conf>, DeltaMut<'_, SubmergeContainer<Conf>>) {
        // Returns both items in one call just to work around partial borrow issues.
        let handle = self.handle.borrow_mut();
        (
            &self.update_context,
            DeltaMut {
                base: Some(&handle.doc.root),
                delta: &mut self.delta,
            },
        )
    }

    /// Commit the modifications in this transaction to the data store.
    ///
    /// ## Returns
    /// * `Ok` if the network update was successfully committed
    /// * [`VersionOverflow`][TransactionError::VersionOverflow] if the update would cause the
    ///   version timestamp to overflow. This happens when the node has made more than [`u32::MAX`]
    ///   changes to the datastore.
    /// * [`StorageError`][TransactionError::StorageError] if it failed to persist the updated data.
    ///   See the documentation for the [`StorageInterface`] implementation for details on how to
    ///   handle the error.
    pub fn commit(mut self) -> Result<(), TransactionError<Conf>> {
        let handle = self.handle.borrow_mut();
        let merged = CrdtState::merge(&handle.doc.root, &self.delta);
        if merged == handle.doc.root {
            info!("No updates were made");
            return Ok(());
        }
        let new_version = handle.doc.version.increment(&handle.metadata.node_id)?;
        let updated_doc = Document {
            root: merged,
            version: new_version.clone(),
        };
        let delta_doc = DeltaDocument {
            root: self.delta,
            base_version: handle.doc.version.clone(),
            version: new_version,
        };
        handle.metadata.network.on_new_update(&handle.id, delta_doc);
        handle
            .metadata
            .storage
            .on_new_update(&handle.id, &updated_doc)
            .map_err(TransactionError::StorageError)?;
        handle.doc = updated_doc;
        Ok(())
    }

    /// Get the update context for the local node in this data store.
    pub fn update_context(&self) -> &UpdateContext<Conf> {
        &self.update_context
    }
}

/// Error type used for [`DocumentHandle::commit_network_update`].
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum CommitUpdateError<Conf: DataStoreConfig> {
    #[error(transparent)]
    StorageError(StorageError<Conf>),
    #[error("Delta update needs to cover to an older base to be applied")]
    OlderBaseNeeded {
        /// The base version needed to successfully update this data store.
        required_base_version: VectorClock<Conf::NodeId>,
    },
}

/// Error type used for [`DocumentHandle::modify`].
#[derive(Error)]
#[derive_where(Debug; E)]
pub enum ModifyError<E, Conf: DataStoreConfig> {
    /// Failed to commit the modifications. See [`TransactionError`].
    #[error(transparent)]
    TransactionFailed(#[from] TransactionError<Conf>),
    /// Custom error type returned from the caller-provided closure.
    Custom(E),
}

type StorageError<Conf> = <<Conf as DataStoreConfig>::Storage as StorageInterface<Conf>>::Error;

impl<Conf> DocumentHandle<Conf>
where
    Conf: DataStoreConfig,
{
    /// Get the document ID for the document.
    pub fn doc_id(&self) -> &DocId {
        &self.id
    }

    /// Get a snapshot of the document.
    pub fn snapshot(&self) -> &Document<Conf> {
        &self.doc
    }

    /// Modify the document.
    ///
    /// This method takes a closure that the caller can use to mutate the document. If the closure
    /// returns success, the mutated document is committed, persisting the result to disk and
    /// sending update messages over the network.
    ///
    /// If the modify closure returns an error or panics, the mutated document is discarded and the
    /// persisted data remains in the previous state.
    pub fn modify<'a, E, F>(&'a mut self, modify: F) -> Result<(), ModifyError<E, Conf>>
    where
        F: for<'d> FnOnce(
            DeltaMut<'d, SubmergeContainer<Conf>>,
            &UpdateContext<Conf>,
        ) -> Result<(), E>,
    {
        let mut transaction = DocumentTransaction::new(self);
        let (ctx, doc) = transaction.get_context_and_root_mut();
        match modify(doc, ctx) {
            Ok(_) => {
                transaction.commit()?;
                Ok(())
            }
            Err(e) => Err(ModifyError::Custom(e)),
        }
    }

    /// Handle an update message from the network.
    ///
    /// Merge any changes received from the incoming update message, and persist the result.
    pub fn commit_network_update(
        &mut self,
        update_message: &DeltaDocument<Conf>,
    ) -> Result<(), CommitUpdateError<Conf>> {
        if let Some(Ordering::Less) | None =
            self.doc.version.partial_cmp(&update_message.base_version)
        {
            // Possible violation of causal consistency: We got a delta update but we are not
            // strictly newer than its base. Return an error to ask the updater for a wider delta.
            return Err(CommitUpdateError::OlderBaseNeeded {
                required_base_version: self.doc.version.clone(),
            });
        }
        if self.doc.version >= update_message.version {
            debug!(
                "Discard obsolete network update. our version={:?}. their version={:?}.",
                self.doc.version, update_message.version,
            );
            return Ok(());
        }
        let updated_document = Document {
            root: CrdtState::merge(&self.doc.root, &update_message.root),
            version: VectorClock::least_upper_bound(&self.doc.version, &update_message.version),
        };
        debug!(
            "Commit network update:\norig={:?}\nupdate={:?}\nnew={:?}",
            self.doc, update_message, updated_document
        );
        self.metadata
            .storage
            .on_new_update(&self.id, &updated_document)
            .map_err(CommitUpdateError::StorageError)?;
        self.doc = updated_document;
        Ok(())
    }
}

/// Contains the necessary metadata to update a document.
#[derive_where(Debug, Clone)]
pub struct NodeMetadata<Conf: DataStoreConfig> {
    network: Conf::Network,
    storage: Conf::Storage,
    node_id: Conf::NodeId,
    timestamp_provider: Conf::TimestampProvider,
}

/// A `DataStore` manages multiple [`Document`]s.
///
/// The `DataStore` is a unit of configuration that shares the same network and storage interfaces.
/// This means that all documents in the same `DataStore` are assumed to be synced with the same
/// group and persisted to the same location.
///
/// As mentioned above, [`Document`] is the unit of causality tracking. Causal consistency is not
/// guaranteed across different [`Documents`][Document] within the same `DataStore`.
#[derive(Debug)]
pub struct DataStore<Conf: DataStoreConfig> {
    metadata: NodeMetadata<Conf>,
}

impl<Conf: DataStoreConfig> DataStore<Conf> {
    /// Create a new data store.
    pub fn new(
        local_id: Conf::NodeId,
        network: Conf::Network,
        storage: Conf::Storage,
        timestamp_provider: Conf::TimestampProvider,
    ) -> Self {
        Self {
            metadata: NodeMetadata {
                network,
                storage,
                node_id: local_id,
                timestamp_provider,
            },
        }
    }

    /// Open the document with the given ID, creating it if it doesn't already exist.
    pub fn open_or_create_document(
        &mut self,
        id: impl Into<DocId>,
    ) -> Result<DocumentHandle<Conf>, StorageError<Conf>>
    where
        SubmergeContainer<Conf>: Default,
    {
        let id = id.into();
        let doc = self
            .metadata
            .storage
            .read_from_storage(&id)?
            .unwrap_or_default();
        Ok(DocumentHandle {
            id,
            doc,
            metadata: self.metadata.clone(),
        })
    }

    /// Get a snapshot of what is currently in the document.
    pub fn snapshot(&self, id: impl Into<DocId>) -> Option<Document<Conf>> {
        let id = id.into();
        self.metadata.storage.read_from_storage(&id).ok()?
    }

    /// Gets the storage interface for this data store.
    pub fn storage(&self) -> &Conf::Storage {
        &self.metadata.storage
    }
}
