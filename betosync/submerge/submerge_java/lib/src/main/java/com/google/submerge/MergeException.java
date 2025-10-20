// Copyright 2025 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

package com.google.android.submerge;

/**
 * Thrown when a CRDT merge fails.
 *
 * <p>A merge can fail when some of the peers do not follow the expected algorithm for generating
 * update messages, or fail to validate for necessary preconditions for mutation operations.
 * Examples include sending different updates with the same timestamp, creating a data store from
 * corrupted update messages or persisted bytes, or different peers creating their data store from
 * conflicting schemas.
 */
public class MergeException extends Exception {
    MergeException(String message) {
        super(message);
    }
}
