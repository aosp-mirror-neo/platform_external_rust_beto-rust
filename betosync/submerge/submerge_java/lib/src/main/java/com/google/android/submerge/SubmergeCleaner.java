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

import androidx.annotation.NonNull;
import com.google.android.cooperativecleaner.CooperativeCleaner;
import com.google.android.cooperativecleaner.CooperativeCleaner.Registration;

public final class SubmergeCleaner {
  private static final CooperativeCleaner cleaner = new CooperativeCleaner();

  /** Gets the singleton instance of {@link CooperativeCleaner} used in the submerge library. */
  public static CooperativeCleaner cleaner() {
    return cleaner;
  }

  /**
   * A cleanable type that can be used with a {@link CooperativeCleaner}.
   *
   * <p>If an object {@code Foo} owns a resource {@code bar} that needs to be cleaned, instead of
   * directly storing {@code bar} in a field, {@code Foo} would instead define a {@code static class
   * CleanableBar extends Cleanable} and hold that in a private field instead.
   *
   * <p>This object must not have references to the {@code owner} to allow the owner to be garbage
   * collected. To avoid accidentally creating those references, {@code Bar} should be static when
   * defined as a nested class.
   */
  public abstract static class Cleanable implements AutoCloseable {
    @NonNull private final Registration registration;

    /**
     * Creates a cleanable instance.
     *
     * <p>This will tie the lifetime of this {@code Cleanable} with the {@code owner} such that when
     * the {@code owner} gets garbage collected, {@link #clean()} will (eventually) be called to
     * free up the underlying resource.
     */
    protected Cleanable(Object owner) {
      registration = SubmergeCleaner.cleaner().register(owner, this::clean);
    }

    /** Cleans up any resources owned by this object. */
    protected abstract void clean();

    /**
     * {@inheritDoc}
     *
     * <p>{@link #close} is different from {@link #clean} in that {@link #close} will also
     * cooperatively clean up any phantom references that are waiting to be cleaned.
     */
    @Override
    public void close() {
      registration.close();
    }
  }
}
