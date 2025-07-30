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
import java.util.ArrayList;
import java.util.List;

/**
 * A distributed register, which is a container that updates and merges atomically.
 *
 * <p>The value with the latest timestamp in the register can be retrieved using {@link #get()}. To
 * get all concurrent values in the register (sometimes called "conflicts"), use {@link #getAll()}.
 *
 * <p>When {@link #set} is called, it will overwrite all values in the set with the new value
 * (sometimes referred to as "resolving the conflict").
 */
public class SubmergeRegister<T> extends SubmergeDataType<T> {
  SubmergeRegister(@SubmergeDataTypeHandleId long handle, Converter<T> converter) {
    super(handle, converter);
  }

  /**
   * Sets the value of this register.
   *
   * <p>If there are concurrent updates to the register, all values are kept and can be retrieved
   * using {@link #getAll()}. When {@link #get()} is used, the value with the latest associated
   * timestamp is returned.
   *
   * <p>This overrides all values currently in the register, implicitly resolving all "conflicts"
   * from concurrent modifications that happened before.
   *
   * @param value the value to be set in this register
   * @throws TimestampOverflowException if the timestamp overflowed in the process of setting this
   *     new value. When this happens the register retains the old value. This happens when there
   *     are more than 255 modifications in one second, or if the wall clock timestamp is larger
   *     than 2^32 seconds since Unix epoch.
   */
  public void set(T value) throws TimestampOverflowException {
    nativeSet(this.getHandle(), this.converter.serialize(value));
  }

  /**
   * Returns the latest value in this register.
   *
   * <p>{@code null} may be returned if this register has not been initialized. Otherwise, the
   * latest value in this register, according to the timestamp, is returned. (Last-writer-wins
   * semantics).
   */
  @Nullable
  public T get() {
    byte[] rawValue = nativeGet(this.getHandle());
    return this.converter.deserialize(rawValue);
  }

  /**
   * Returns the list of all values concurrently added in this register.
   *
   * <p>This can be thought of as the list of all "conflicting" values in this register. The
   * returned list is ordered from newest to oldest, meaning that in the typical case {@code
   * getAll().get(0)} is the same as {@code get()}.
   */
  public List<T> getAll() {
    byte[][] rawList = nativeGetAll(this.getHandle());
    List<T> result = new ArrayList<>(rawList.length);
    for (byte[] rawItem : rawList) {
      result.add(this.converter.deserialize(rawItem));
    }
    return result;
  }

  @Override
  public synchronized String toString() {
    return "SubmergeRegister {" + super.toString() + "}";
  }

  static {
    System.loadLibrary("submerge_java");
  }

  private static native void nativeSet(@SubmergeDataTypeHandleId long handle, byte[] value)
      throws TimestampOverflowException;

  private static native byte[] nativeGet(@SubmergeDataTypeHandleId long handle);

  private static native byte[][] nativeGetAll(@SubmergeDataTypeHandleId long handle);
}
