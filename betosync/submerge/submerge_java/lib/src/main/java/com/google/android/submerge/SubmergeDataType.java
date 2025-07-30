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
import androidx.annotation.IntDef;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import com.google.android.submerge.SubmergeCleaner.Cleanable;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/** An abstract base class for data types supported by Submerge's data model. */
public abstract class SubmergeDataType<T> implements AutoCloseable {

  @IntDef({
    CrdtType.CRDT_TYPE_NONE,
    CrdtType.CRDT_TYPE_MAP,
    CrdtType.CRDT_TYPE_SET,
    CrdtType.CRDT_TYPE_REGISTER,
    CrdtType.CRDT_TYPE_VECTOR_DATA,
  })
  @Retention(RetentionPolicy.SOURCE)
  private @interface CrdtType {
    static final int CRDT_TYPE_NONE = 0;
    static final int CRDT_TYPE_MAP = 1;
    static final int CRDT_TYPE_SET = 2;
    static final int CRDT_TYPE_REGISTER = 3;
    static final int CRDT_TYPE_VECTOR_DATA = 4;
  }

  protected final Converter<T> converter;

  /**
   * Annotation on the {@code long} handles that specifics it corresponds to the Rust {@code
   * SubmergeDataTypeReferenceHandle} type.
   *
   * <p>Currently this annotation is not checked by any automated tooling, but something like
   * Checker Framework could be configured in the future.
   */
  @Retention(RetentionPolicy.SOURCE)
  @Target({ElementType.TYPE_USE, ElementType.TYPE_PARAMETER})
  @interface SubmergeDataTypeHandleId {}

  private final SubmergeDataTypeHandle handle;

  protected SubmergeDataType(@SubmergeDataTypeHandleId long handle, Converter<T> converter) {
    this.handle = new SubmergeDataTypeHandle(this, handle);
    this.converter = converter;
  }

  void setHandleLocked(@SubmergeDataTypeHandleId long handle) {
    synchronized (this.handle) {
      this.handle.handle = handle;
    }
  }

  @SubmergeDataTypeHandleId
  long getHandle() {
    return this.handle.get();
  }

  @SubmergeDataTypeHandleId
  long takeHandle() {
    return this.handle.take();
  }

  @Override
  public void close() {
    this.handle.close();
  }

  @Override
  public String toString() {
    synchronized (this.handle) {
      return nativeToString(this.handle.handle);
    }
  }

  @Nullable
  static <T> SubmergeDataType<T> create(
      @SubmergeDataTypeHandleId long handle, @NonNull Converter<T> converter) {
    @CrdtType int type = nativeGetCrdtType(handle);
    switch (type) {
      case CrdtType.CRDT_TYPE_NONE:
        return null;
      case CrdtType.CRDT_TYPE_MAP:
        return new SubmergeMap<>(handle, converter);
      case CrdtType.CRDT_TYPE_SET:
        return new SubmergeSet<>(handle, converter);
      case CrdtType.CRDT_TYPE_REGISTER:
        return new SubmergeRegister<>(handle, converter);
      case CrdtType.CRDT_TYPE_VECTOR_DATA:
        return new SubmergeVectorData<>(handle, converter);
      default:
        throw new RuntimeException("Unrecognized type");
    }
  }

  private static final class SubmergeDataTypeHandle extends Cleanable {
    @GuardedBy("this")
    private @SubmergeDataTypeHandleId long handle = -1;

    SubmergeDataTypeHandle(Object owner, @SubmergeDataTypeHandleId long handle) {
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

    synchronized @SubmergeDataTypeHandleId long take() {
      long handle = this.handle;
      this.handle = -1;
      return handle;
    }

    synchronized @SubmergeDataTypeHandleId long get() {
      return this.handle;
    }
  }

  static {
    System.loadLibrary("submerge_java");
  }

  @CrdtType
  private static native int nativeGetCrdtType(@SubmergeDataTypeHandleId long handle);

  private static native void nativeClose(@SubmergeDataTypeHandleId long handle);

  private static native String nativeToString(@SubmergeDataTypeHandleId long handle);
}
