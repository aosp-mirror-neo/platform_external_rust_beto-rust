/*
 * Copyright 2024 Google LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

package com.google.android.submerge;

import androidx.annotation.GuardedBy;
import androidx.annotation.Nullable;
import com.google.android.submerge.DocumentTransaction.DocumentTransactionHandleId;
import com.google.android.submerge.SubmergeCleaner.Cleanable;
import com.google.android.submerge.VersionVector.VersionVectorHandleId;
import com.google.errorprone.annotations.MustBeClosed;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/**
 * A distributed data store.
 *
 * <p>The data store manages multiple documents, and is a unit of configuration that shares the same
 * network and storage interfaces. This means that all documents in the same data store are assumed
 * to be synced with the same group and persisted to the same location.
 *
 * <p>As mentioned above, document (see {@link DocumentTransaction}) is the unit of causality
 * tracking. Causal consistency is not guaranteed across different documents within the same data
 * store.
 */
public class DataStore<T> implements AutoCloseable {

  /** The handle used as a key to find the associated native {@code DataStore} in a handle map. */
  private final DataStoreHandle handle;

  /** The converter used to convert between the value type {@code T} and {@code byte[]}. */
  private final Converter<T> converter;

  /**
   * Creates a new data store.
   *
   * @param localId The local ID identifying this local nodes within the group. This ID must be
   *     unique amongst all nodes that syncs this data store, including nodes that used to, but no
   *     longer, participate in the sync.
   * @param networkInterface The network interface the is responsible for dispatching update
   *     messages to other nodes in the syncing group when new transactions are committed to the
   *     data store.
   * @param storageInterface The storage interface that is responsible for persisting the contents
   *     of the data store onto disk.
   * @param timestampProvider Provider for the wall clock time.
   * @param converter The converter used to convert between the value type {@code T} and {@code
   *     byte[]}.
   */
  @MustBeClosed
  public DataStore(
      String localId,
      NetworkInterface networkInterface,
      StorageInterface storageInterface,
      TimestampProvider timestampProvider,
      Converter<T> converter) {
    @DataStoreHandleId
    long handle = nativeInit(localId, networkInterface, storageInterface, timestampProvider);
    this.handle = new DataStoreHandle(this, handle);
    this.converter = converter;
  }

  /**
   * Creates a new document transaction.
   *
   * <p>Creates a new document transaction for the given {@code docId}. If a document for the given
   * {@code docId} already exists, it will open that document and create a transaction for it.
   * Otherwise, a new document at that ID is created.
   *
   * @param docId The document ID to open or create.
   * @return The newly created document transaction that can be used to mutate the document.
   */
  @MustBeClosed
  public DocumentTransaction<T> newDocumentTransaction(String docId) {
    synchronized (this.handle) {
      return new DocumentTransaction<>(
          nativeNewDocumentTransaction(this.handle.handle, docId), docId, this.converter);
    }
  }

  /**
   * Returns a "full" update message.
   *
   * <p>The update message can be applied to any document (including an empty one) via {@link
   * #commitNetworkUpdate(String, byte[])} to catch up to the current state.
   *
   * @param docId The document ID to get the full update message.
   */
  @Nullable
  public byte[] getFullUpdateMessage(String docId) {
    synchronized (this.handle) {
      return nativeGetFullUpdateMessage(this.handle.handle, docId);
    }
  }

  /**
   * Returns the current version for the document with {@code docId}.
   *
   * <p>The returned version can be used to {@link #calculateDelta(String, VersionVector)} on a peer
   * to get an update message to catch our data store up to the peer's version.
   */
  public VersionVector getDocumentVersion(String docId) {
    synchronized (this.handle) {
      return new VersionVector(nativeGetDocumentVersion(this.handle.handle, docId));
    }
  }

  /**
   * Calculates the delta needed to update from the given {@code baseVersion}.
   *
   * <p>The {@code baseVersion} must be a version vector from a replica of the same document. The
   * returned document can be applied using {@link #commitNetworkUpdate(String, byte[])} on a
   * document that is at least as new as {@code baseVersion}.
   */
  @Nullable
  public byte[] calculateDelta(String docId, VersionVector baseVersion) {
    synchronized (this.handle) {
      return nativeCalculateDelta(this.handle.handle, docId, baseVersion.getHandle());
    }
  }

  /**
   * Commits changes from a given network update message.
   *
   * @param docId the ID of the document to which the update message belongs to.
   * @param updateMessage the update message previously given to {@link
   *     NetworkInterface#onNewUpdate(String, byte[])} on a remote node.
   * @throws TransactionException if the transaction cannot be committed, for example if there are
   *     failures while trying to write to disk.
   * @throws OlderBaseNeededException if the {@code updateMessage} given doesn't cover an old enough
   *     base to be applied.
   * @throws RuntimeException if this transaction has already been committed.
   */
  public void commitNetworkUpdate(String docId, byte[] updateMessage)
      throws TransactionException, OlderBaseNeededException {
    synchronized (this.handle) {
      nativeCommitNetworkUpdate(this.handle.handle, docId, updateMessage);
    }
  }

  @Override
  public void close() {
    this.handle.close();
  }

  private static final class DataStoreHandle extends Cleanable {

    @GuardedBy("this")
    private @DataStoreHandleId long handle;

    DataStoreHandle(Object owner, @DataStoreHandleId long handle) {
      super(owner);
      this.handle = handle;
    }

    @Override
    public synchronized void clean() {
      if (this.handle != -1) {
        nativeClose(this.handle);
        this.handle = -1;
      }
    }
  }

  /**
   * Annotation on the {@code long} handles that specifics it corresponds to the Rust {@code
   * DataStoreHandle} type.
   *
   * <p>Currently this annotation is not checked by any automated tooling, but something like
   * Checker Framework could be configured in the future.
   */
  @Retention(RetentionPolicy.SOURCE)
  @Target({ElementType.TYPE_USE, ElementType.TYPE_PARAMETER})
  @interface DataStoreHandleId {}

  static {
    System.loadLibrary("submerge_java");
  }

  private static native @DataStoreHandleId long nativeInit(
      String localId,
      NetworkInterface networkInterface,
      StorageInterface storageInterface,
      TimestampProvider timestampProvider);

  private static native void nativeClose(@DataStoreHandleId long handle);

  private static native @DocumentTransactionHandleId long nativeNewDocumentTransaction(
      @DataStoreHandleId long handle, String docId);

  @Nullable
  private static native byte[] nativeGetFullUpdateMessage(
      @DataStoreHandleId long handle, String docId);

  @Nullable
  private static native byte[] nativeCalculateDelta(
      @DataStoreHandleId long handle, String docId, @VersionVectorHandleId long baseVersionHandle);

  private static native void nativeCommitNetworkUpdate(
      @DataStoreHandleId long handle, String docId, byte[] updateMessage);

  private static native @VersionVectorHandleId long nativeGetDocumentVersion(
      @DataStoreHandleId long handle, String docId);
}
