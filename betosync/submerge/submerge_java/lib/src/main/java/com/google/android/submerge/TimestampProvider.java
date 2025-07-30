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

import org.checkerframework.checker.signedness.qual.Unsigned;

/**
 * A provider for the current wall clock time.
 *
 * <p>This wall-clock is typically used as a tie-breaker for concurrent changes in last-writer-wins
 * scenarios. The clock may go backwards in time (not required to be monotonic). A skewed timestamp
 * may unfairly bias concurrent changes to a particular node, and may affect future updates' ability
 * to effectively tie-break using the timestamp, but it will not affect data consistency.
 *
 * <p>(In the worst case, a heavily skewed clock will cause the timestamps to behave as if they are
 * Lamport timestamps.)
 */
public interface TimestampProvider {
  /**
   * Returns the number of milliseconds since Unix epoch. This number is treated as an unsigned
   * long. (See the {@code *Unsigned} functions in {@link Long}).
   */
  @Unsigned
  long now();
}
