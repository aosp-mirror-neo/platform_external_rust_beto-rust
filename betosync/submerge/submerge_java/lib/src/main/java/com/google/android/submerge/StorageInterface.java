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

import androidx.annotation.Nullable;
import java.io.IOException;

/**
 * The storage interface for a data store.
 *
 * <p>When a {@link DocumentTransaction} is committed, or when new update messages are received from
 * the network, {@link #onNewUpdate(String, byte[])} will be called to request the storage layer to
 * persist the latest state.
 *
 * <p>If there are failures persisting to disk, implementations should throw a {@link
 * StorageException}, which will cause the corresponding {@link DocumentTransaction#commit()} to
 * throw {@link TransactionException}.
 */
public interface StorageInterface {

  /** Exception thrown when the storage failed to persist the given document. */
  public static class StorageException extends IOException {}

  /**
   * Called when a new update to the data store should be persisted to storage.
   *
   * <p>If this call returned without throwing an exception, subsequent calls to {@link
   * #readFromStorage} should return {@code serializedDoc} unless overwritten by another call to
   * this method.
   *
   * @param docId the ID of the document associated with the document.
   * @param serializedDoc the serialized document to be persisted.
   * @throws StorageException if the persist operation failed.
   */
  void onNewUpdate(String docId, byte[] serializedDoc) throws StorageException;

  /**
   * Reads a serialized document from storage.
   *
   * @param docId The ID of the document to read from storage.
   * @return the document as previously passed to {@link #onNewUpdate(String, byte[])}, or {@code
   *     null} if there are no documents with the given {@code docId}.
   * @throws StorageException if it failed to read from storage.
   */
  @Nullable
  byte[] readFromStorage(String docId) throws StorageException;
}
