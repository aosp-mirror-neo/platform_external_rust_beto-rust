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
 * Exception that is thrown when ownership requirements are violated.
 *
 * <p>Some of the Submerge APIs have ownership requirements such that the same object cannot be used
 * multiple times. These are programmer errors — these exceptions should be fixed by modifying the
 * code rather than adding try-catch blocks.
 */
public class OwnershipException extends RuntimeException {
  OwnershipException(String msg) {
    super(msg);
  }
}
