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

/**
 * Exception thrown when trying to commit a network update but the delta update doesn't cover the
 * required base version. {@link #getBaseVersionNeeded()} can be used to retrieve the required base
 * version, which can then be passed to {@link DataStore#calculateDelta(VersionVector)} to calculate
 * a recent enough update message.
 */
public class OlderBaseNeededException extends Exception {
  private final VersionVector baseVersionNeeded;

  OlderBaseNeededException(VersionVector baseVersionNeeded) {
    super("Update message with older base version is needed");
    this.baseVersionNeeded = baseVersionNeeded;
  }

  /**
   * Returns the base version that is needed to update this database.
   */
  public VersionVector getBaseVersionNeeded() {
    return this.baseVersionNeeded;
  }
}
