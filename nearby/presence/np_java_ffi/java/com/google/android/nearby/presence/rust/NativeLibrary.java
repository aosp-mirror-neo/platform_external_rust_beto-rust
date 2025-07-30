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

package com.google.android.nearby.presence.rust;

/**
 * Represents a native library to be loaded. Call {@link #ensureLoaded()} to ensure that the library
 * has been loaded.
 */
public class NativeLibrary {
  private final String libraryName;

  /**
   * Create an instance. {@code libraryName} should be the same name that would be passed to {@link
   * System#loadLibrary(String)}.
   */
  public NativeLibrary(String libraryName) {
    this.libraryName = libraryName;
  }

  /** Ensure the library is loaded by loading it if needed. */
  public void ensureLoaded() {
    System.loadLibrary(this.libraryName);
  }
}
