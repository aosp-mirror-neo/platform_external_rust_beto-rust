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
import com.google.android.submerge.SubmergeCleaner.Cleanable;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

public final class VersionVector implements AutoCloseable {

  private final VersionVectorHandle handle;

  @SuppressWarnings("unused") // Also used by native
  VersionVector(@VersionVectorHandleId long handle) {
    this.handle = new VersionVectorHandle(this, handle);
  }

  @Override
  public void close() {
    this.handle.close();
  }

  @VersionVectorHandleId
  long getHandle() {
    return this.handle.get();
  }

  private static final class VersionVectorHandle extends Cleanable {
    @GuardedBy("this")
    private @VersionVectorHandleId long handle;

    VersionVectorHandle(Object owner, @VersionVectorHandleId long handle) {
      super(owner);
      this.handle = handle;
    }

    @Override
    protected synchronized void clean() {
      if (this.handle != -1) {
        nativeClose(this.handle);
        this.handle = -1;
      }
    }

    private synchronized @VersionVectorHandleId long get() {
      return this.handle;
    }
  }

  /**
   * Annotation on the {@code long} handles that specifics it corresponds to the Rust {@code
   * VersionVectorHandle} type.
   *
   * <p>Currently this annotation is not checked by any automated tooling, but something like
   * Checker Framework could be configured in the future.
   */
  @Retention(RetentionPolicy.SOURCE)
  @Target({ElementType.TYPE_USE, ElementType.TYPE_PARAMETER})
  @interface VersionVectorHandleId {}

  private static native void nativeClose(@VersionVectorHandleId long handle);
}
