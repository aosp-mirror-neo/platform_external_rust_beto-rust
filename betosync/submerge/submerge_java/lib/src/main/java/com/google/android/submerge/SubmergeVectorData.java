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

import androidx.annotation.VisibleForTesting;
import com.google.errorprone.annotations.CanIgnoreReturnValue;
import java.util.HashMap;
import java.util.Map;
import org.checkerframework.checker.signedness.qual.Unsigned;

/**
 * A vector of data with one entry per node, where each node can only modify their own entry.
 *
 * <p>The name draws similarity to how "vector clock" is a vector of logical clocks and how "version
 * vector" is a vector of version numbers.
 *
 * <p>The data is tagged with a monotonically increasing version number, which is guaranteed to be
 * unique because there is only a single writer.
 *
 * <p>When this type is merged, the vector is merged entry-by-entry, taking the entry with the
 * higher version number. Since the local version number is always incremented on data change, an
 * equal version number must imply that the data is also equal.
 */
public class SubmergeVectorData<T> extends SubmergeDataType<T> {
  SubmergeVectorData(@SubmergeDataTypeHandleId long handle, Converter<T> converter) {
    super(handle, converter);
  }

  /**
   * Sets the value to {@code value} in the entry associated with the local node.
   *
   * @return {@code true} if the value was inserted or updated, or {@code false} if {@code value} is
   *     already equal to the entry in the vector.
   * @throws VersionOverflowException if the version number for the local node will overflow, which
   *     means that this entry has been modified 2^32 times.
   */
  @CanIgnoreReturnValue
  public boolean set(T value) throws VersionOverflowException {
    return nativeSet(this.getHandle(), this.converter.serialize(value));
  }

  /**
   * Bumps the version number for testing, setting the value to {@code null}.
   *
   * @param increment the amount to increase the version number by.
   * @throws VersionOverflowException if the version number would overflow after increasing by
   *     {@code increment}.
   */
  @VisibleForTesting
  public void bumpVersionNumberForTesting(@Unsigned int increment) throws VersionOverflowException {
    nativeBumpVersionNumberForTesting(this.getHandle(), increment);
  }

  /**
   * Returns the map of entries in this vector data.
   *
   * <p>The key of the map are the node IDs, and the values are the data set previously using {@link
   * #set}.
   */
  public Map<String, T> entries() {
    Map<String, byte[]> rawEntries = nativeEntries(this.getHandle());
    Map<String, T> entries = new HashMap<>(rawEntries.size());
    for (Map.Entry<String, byte[]> entry : rawEntries.entrySet()) {
      entries.put(entry.getKey(), this.converter.deserialize(entry.getValue()));
    }
    return entries;
  }

  @Override
  public synchronized String toString() {
    return "SubmergeVectorData {" + super.toString() + "}";
  }

  static {
    System.loadLibrary("submerge_java");
  }

  private static native boolean nativeSet(@SubmergeDataTypeHandleId long handle, byte[] value)
      throws VersionOverflowException;

  private static native void nativeBumpVersionNumberForTesting(
      @SubmergeDataTypeHandleId long handle, @Unsigned int increment) throws VersionOverflowException;

  private static native Map<String, byte[]> nativeEntries(@SubmergeDataTypeHandleId long handle);
}
