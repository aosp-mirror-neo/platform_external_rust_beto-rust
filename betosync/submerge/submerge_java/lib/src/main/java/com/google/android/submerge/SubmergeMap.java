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

/**
 * A distributed map type.
 *
 * <p>Keys to this map are {@link String Strings} and values are {@link SubmergeDataType
 * SubmergeDataTypes}.
 *
 * <p>This is the main structural element of Submerge's data model, and may be used to encode a the
 * schema of the document.
 *
 * <p>When there are concurrent {@link #setChild(String, SubmergeDataType)} calls at the same key,
 * this map merges them using last-writer-wins via a modified version of the wall-clock timestamp.
 */
// TODO: Add schema example to the documentation.
public class SubmergeMap<T> extends SubmergeDataType<T> {
  SubmergeMap(@SubmergeDataTypeHandleId long handle, Converter<T> converter) {
    super(handle, converter);
  }

  /**
   * Returns the child at the given key, or {@code null} if the value does not exist.
   *
   * <p>Modifications to this child will be merged according to the child's merging rules.
   */
  @Nullable
  public SubmergeDataType<T> getChild(String key) {
    @SubmergeDataTypeHandleId long handle = nativeGetChild(this.getHandle(), key);
    return SubmergeDataType.create(handle, converter);
  }

  /**
   * Sets the child value at the given key.
   *
   * <p>If there are concurrent modifications to the child value (via {@link #getChild(String)}),
   * this new {@code child} will be used and the remote modifications via {@code getChild} are
   * discarded.
   *
   * <p>If two nodes concurrently modify the same key using {@link #setChild(String,
   * SubmergeDataType)}, the writer with a larger timestamp value is more likely to win.
   *
   * @param key the key to set the child at.
   * @param child the child to be set at the given key. This {@code child} must be owned by the
   *     caller, typically just created using, say, {@link DataStore#newMap()}.
   * @throws TimestampOverflowException if the timestamp overflowed in the process of setting this
   *     new child. When this happens the old child is retained. This happens when there are more
   *     than 255 modifications in one second, or if the timestamp is larger than 2^32 seconds since
   *     Unix epoch.
   * @throws OwnershipException if the {@code child} passed in is not owned by the caller,
   *     indicating a programmer error. See the documentation for {@code child} above.
   */
  public void setChild(String key, SubmergeDataType<T> child) throws TimestampOverflowException {
    synchronized (child) {
      @SubmergeDataTypeHandleId
      long childHandle = nativeSetChild(this.getHandle(), key, child.getHandle());
      child.setHandleLocked(childHandle);
    }
  }

  /**
   * Removes the child value at the given key.
   *
   * <p>This merges using the same rules as {@link #setChild(String, SubmergeDataType)}, and is
   * equivalent to {@code setChild(key, null)}.
   *
   * @param key the key to set the child at.
   * @throws TimestampOverflowException if the timestamp overflowed in the process of removing this
   *     child. When this happens the old child is retained. This happens when there are more than
   *     255 modifications in one second, or if the timestamp is larger than 2^32 seconds since Unix
   *     epoch.
   */
  public void removeChild(String key) throws TimestampOverflowException {
    nativeRemoveChild(this.getHandle(), key);
  }

  @Override
  public synchronized String toString() {
    return "SubmergeMap {" + super.toString() + "}";
  }

  static {
    System.loadLibrary("submerge_java");
  }

  private static native @SubmergeDataTypeHandleId long nativeGetChild(
      @SubmergeDataTypeHandleId long handle, String key);

  private static native @SubmergeDataTypeHandleId long nativeSetChild(
      @SubmergeDataTypeHandleId long handle, String key, @SubmergeDataTypeHandleId long childHandle)
      throws TimestampOverflowException;

  private static native void nativeRemoveChild(@SubmergeDataTypeHandleId long handle, String key)
      throws TimestampOverflowException;
}
