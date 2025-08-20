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
import com.google.android.submerge.SubmergeCleaner.Cleanable;
import com.google.android.submerge.SubmergeDataType.SubmergeDataTypeHandleId;
import com.google.errorprone.annotations.CanIgnoreReturnValue;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/**
 * A transaction for a document.
 *
 * <p>A document is the unit of causality tracking in Submerge. This means that if a node makes a
 * modification A to the document, all other syncing nodes are guaranteed to observe all events that
 * happened before A before they observe modification A.
 *
 * <p>The transaction temporarily stores any modifications to the underlying document. When {@link
 * #commit()} is called, those changes are committed to storage and dispatched to the network. If
 * this object is freed without calling {@link #commit()}, this transaction is considered aborted.
 */
public class DocumentTransaction<T> implements AutoCloseable {

  private final String docId;
  private final Converter<T> converter;
  private final DocumentTransactionHandle handle;

  DocumentTransaction(
      @DocumentTransactionHandleId long handle, String docId, Converter<T> converter) {
    this.handle = new DocumentTransactionHandle(this, handle);
    this.docId = docId;
    this.converter = converter;
  }

  /**
   * Returns the root of this document.
   *
   * <p>Modifications to the returned value are kept within this transaction until {@link #commit()}
   * is called.
   *
   * <p>The root document may be {@code null}, or one of the implementations of {@link
   * SubmergeDataType}. To allow future evolution of the document schema, you may want to set a
   * {@link SubmergeMap} at the root.
   */
  @Nullable
  public SubmergeDataType<T> getRoot() {
    synchronized (this.handle) {
      @SubmergeDataTypeHandleId long rootHandle = nativeGetRoot(this.handle.handle);
      return SubmergeDataType.create(rootHandle, this.converter);
    }
  }

  /**
   * Sets the root of this document.
   *
   * <p>Modifications are kept within this transaction until {@link #commit()} is called.
   *
   * <p>To allow future evolution of the document schema, you may want to set a {@link SubmergeMap}
   * at the root.
   *
   * @param root The new root. Can be {@code null}, in which case this node will become empty. This
   *     {@code root} passed in must be owned by the caller, typically just created using, say,
   *     {@link DataStore#newMap()}.
   * @throws TimestampOverflowException if the timestamp overflowed in the process of setting this
   *     new child. When this happens the old child is retained. This happens when there are more
   *     than 255 modifications in one second, or if the timestamp is larger than 2^32 seconds since
   *     Unix epoch.
   * @throws OwnershipException if the {@code root} passed in is not owned by the caller, indicating
   *     a programmer error. See the documentation for {@code root} above.
   */
  public void setRoot(@Nullable SubmergeDataType<T> root) throws TimestampOverflowException {
    synchronized (this.handle) {
      if (root != null) {
        @SubmergeDataTypeHandleId
        long rootHandle = nativeSetRoot(this.handle.handle, root.takeHandle());
        root.setHandleLocked(rootHandle);
      } else {
        nativeSetRoot(this.handle.handle, -1);
      }
    }
  }

  /**
   * Creates a new register for this document.
   *
   * <p>After initialization the register should be added into the document using {@link
   * DocumentTransaction#setRoot(SubmergeDataType)} or {@link SubmergeMap#setChild(String,
   * SubmergeDataType)}.
   *
   * <p>This register must only be used within this document.
   *
   * @see SubmergeRegister
   */
  public SubmergeRegister<T> newRegister() {
    synchronized (this.handle) {
      if (this.handle.handle == -1) {
        throw new IllegalStateException("Cannot create new data types while closed");
      }
      return new SubmergeRegister<>(nativeNewRegister(this.handle.handle), converter);
    }
  }

  /**
   * Creates a new map for this document.
   *
   * <p>After initialization the map should be added into the document using {@link
   * DocumentTransaction#setRoot(SubmergeDataType)} or {@link SubmergeMap#setChild(String,
   * SubmergeDataType)}.
   *
   * <p>This map must only be used within this document.
   *
   * @see SubmergeMap
   */
  public SubmergeMap<T> newMap() {
    synchronized (this.handle) {
      if (this.handle.handle == -1) {
        throw new IllegalStateException("Cannot create new data types while closed");
      }
      return new SubmergeMap<>(nativeNewMap(this.handle.handle), converter);
    }
  }

  /**
   * Creates a new set for this document.
   *
   * <p>After initialization the set should be added into the document using {@link
   * DocumentTransaction#setRoot(SubmergeDataType)} or {@link SubmergeMap#setChild(String,
   * SubmergeDataType)}.
   *
   * <p>This set must only be used within this document.
   *
   * @see SubmergeSet
   */
  public synchronized SubmergeSet<T> newSet() {
    synchronized (this.handle) {
      if (this.handle.handle == -1) {
        throw new IllegalStateException("Cannot create new data types while closed");
      }
      return new SubmergeSet<>(nativeNewSet(this.handle.handle), converter);
    }
  }

  /**
   * Creates a new vector data for this document.
   *
   * <p>After initialization the vector data should be added into the document using {@link
   * DocumentTransaction#setRoot(SubmergeDataType)} or {@link SubmergeMap#setChild(String,
   * SubmergeDataType)}.
   *
   * <p>This vector data must only be used within this document.
   *
   * @see SubmergeVectorData
   */
  public synchronized SubmergeVectorData<T> newVectorData() {
    synchronized (this.handle) {
      if (this.handle.handle == -1) {
        throw new IllegalStateException("Cannot create new data types while closed");
      }
      return new SubmergeVectorData<>(nativeNewVectorData(this.handle.handle), converter);
    }
  }

  /** Returns the ID of the document for this transaction. */
  public String getDocumentId() {
    return this.docId;
  }

  /**
   * Commits this transaction to the document.
   *
   * <p>The transaction can only be committed (at most) once.
   *
   * @throws TransactionException if the transaction cannot be committed, for example if there are
   *     failures while trying to write to disk.
   * @throws RuntimeException if this transaction has already been committed.
   */
  public void commit() throws TransactionException {
    synchronized (this.handle) {
      nativeCommit(this.handle.takeHandle());
    }
  }

  /**
   * Merges an update from the network into the current transaction.
   *
   * @param update The update message from the network.
   * @throws OlderBaseNeededException if the {@code updateMessage} given doesn't cover an old enough
   *     base to be applied.
   */
  public void mergeNetworkUpdate(byte[] update) throws OlderBaseNeededException {
    synchronized (this.handle) {
      nativeMergeNetworkUpdate(this.handle.handle, update);
    }
  }

  @Override
  public String toString() {
    synchronized (this.handle) {
      if (this.handle.handle == -1) {
        return "DocumentTransaction { handle=null }";
      } else {
        return "DocumentTransaction { handle=" + nativeToString(this.handle.handle) + " }";
      }
    }
  }

  @Override
  public synchronized void close() {
    this.handle.close();
  }

  private static final class DocumentTransactionHandle extends Cleanable {
    /** A handle to the native {@code DocumentTransaction}. */
    @GuardedBy("this")
    private @DocumentTransactionHandleId long handle;

    DocumentTransactionHandle(Object owner, @DocumentTransactionHandleId long handle) {
      super(owner);
      this.handle = handle;
    }

    @Override
    public synchronized void clean() {
      if (this.handle != -1) {
        nativeClose(takeHandle());
        this.handle = -1;
      }
    }

    @GuardedBy("this")
    private @DocumentTransactionHandleId long takeHandle() {
      long handle = this.handle;
      this.handle = -1;
      return handle;
    }
  }

  /**
   * Annotation on the {@code long} handles that specifics it corresponds to the Rust {@code
   * DocumentTransactionHandle} type.
   *
   * <p>Currently this annotation is not checked by any automated tooling, but something like
   * Checker Framework could be configured in the future.
   */
  @Retention(RetentionPolicy.SOURCE)
  @Target({ElementType.TYPE_USE, ElementType.TYPE_PARAMETER})
  @interface DocumentTransactionHandleId {}

  static {
    System.loadLibrary("submerge_java");
  }

  private static native @SubmergeDataTypeHandleId long nativeGetRoot(
      long documentTransactionHandle);

  @CanIgnoreReturnValue
  private static native @SubmergeDataTypeHandleId long nativeSetRoot(
      long documentTransactionHandle, long ownedHandle) throws TimestampOverflowException;

  private static native void nativeCommit(@DocumentTransactionHandleId long handle)
      throws TransactionException;

  private static native void nativeMergeNetworkUpdate(
      @DocumentTransactionHandleId long handle, byte[] update)
      throws OlderBaseNeededException;

  private static native void nativeClose(@DocumentTransactionHandleId long handle);

  private static native String nativeToString(@DocumentTransactionHandleId long handle);

  private static native @SubmergeDataTypeHandleId long nativeNewMap(
      @DocumentTransactionHandleId long handle);

  private static native @SubmergeDataTypeHandleId long nativeNewRegister(
      @DocumentTransactionHandleId long handle);

  private static native @SubmergeDataTypeHandleId long nativeNewSet(
      @DocumentTransactionHandleId long handle);

  private static native @SubmergeDataTypeHandleId long nativeNewVectorData(
      @DocumentTransactionHandleId long handle);
}