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
import java.util.HashSet;
import java.util.Set;
import org.checkerframework.checker.signedness.qual.Unsigned;

/**
 * A distributed set.
 *
 * <p>When there are concurrent modifications on the same value, they are merged by comparing number
 * of add and remove operations on that value. Since multiple consecutive "add" operations has the
 * same effect as adding once in a set, it is treated as one "add" operation. So the number of "add"
 * operations and the number of "remove" operations differ by at most one. If there are more "adds"
 * than "removes", then the value is considered added. Otherwise if they are the same, then the
 * value is considered removed.
 *
 * <p>The underlying implementation is a "Causal Length Set". See the Rust documentation for more
 * details on the implementation.
 */
public class SubmergeSet<T> extends SubmergeDataType<T> {
  SubmergeSet(@SubmergeDataTypeHandleId long handle, Converter<T> converter) {
    super(handle, converter);
  }

  /**
   * Adds {@code value} into the set.
   *
   * <p>If {@code value} is already in the set, this does nothing.
   *
   * @return {@code true} if the set has changed. {@code false} if the value is already in the set.
   * @throws GenerationExhaustedException if the generation number for this item is exhausted,
   *     meaning that this item has been added and removed 2^32 times.
   */
  @CanIgnoreReturnValue
  public boolean add(T value) throws GenerationExhaustedException {
    return nativeAdd(this.getHandle(), this.converter.serialize(value));
  }

  /**
   * Removes {@code value} from the set.
   *
   * <p>If {@code value} is not in the set, this does nothing.
   *
   * @return {@code true} if the value is removed. {@code false} if the value was not in the set to
   *     begin with.
   */
  @CanIgnoreReturnValue
  public boolean remove(T value) {
    return nativeRemove(this.getHandle(), this.converter.serialize(value));
  }

  /**
   * Bumps the generation number for the given {@code value}.
   *
   * <p>This is exposed only for testing behaviors where the generation number is large or
   * overflows. While calling this will not affect the convergence of the data in the store, it is
   * unnecessary to use this in production.
   *
   * @param value the value that will be added to the set and its associated generation value
   *     bumped.
   * @param increment the increase in generation number.
   * @throws GenerationExhaustedException if the generation number cannot be increased by {@code
   *     increment} because it would overflow.
   */
  @VisibleForTesting
  public void bumpGenerationNumberForTesting(T value, @Unsigned int increment)
      throws GenerationExhaustedException {
    nativeBumpGenerationNumberForTesting(
        this.getHandle(), this.converter.serialize(value), increment);
  }

  /** Returns the list of entries in this set. */
  public Set<T> entries() {
    byte[][] rawEntries = nativeEntries(this.getHandle());
    Set<T> entries = new HashSet<>(rawEntries.length);
    for (byte[] entry : rawEntries) {
      entries.add(this.converter.deserialize(entry));
    }
    return entries;
  }

  /** Returns {@code true} if {@code value} is in the set. */
  public boolean contains(T value) {
    return nativeContains(this.getHandle(), this.converter.serialize(value));
  }

  @Override
  public synchronized String toString() {
    return "SubmergeSet {" + super.toString() + "}";
  }

  static {
    System.loadLibrary("submerge_java");
  }

  private static native boolean nativeAdd(@SubmergeDataTypeHandleId long handle, byte[] value)
      throws GenerationExhaustedException;

  private static native boolean nativeRemove(@SubmergeDataTypeHandleId long handle, byte[] value);

  private static native void nativeBumpGenerationNumberForTesting(
      @SubmergeDataTypeHandleId long handle, byte[] value, @Unsigned int increment)
      throws GenerationExhaustedException;

  private static native byte[][] nativeEntries(@SubmergeDataTypeHandleId long handle);

  private static native boolean nativeContains(@SubmergeDataTypeHandleId long handle, byte[] value);
}
