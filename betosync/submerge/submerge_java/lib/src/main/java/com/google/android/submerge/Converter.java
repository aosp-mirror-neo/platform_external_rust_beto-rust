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
 * A converter for {@link DataStore} that serializes and deserializes the value type {@code V} into
 * bytes. The data store only considers equality of the underlying bytes, so this converter and the
 * type implementation `V` should ensure that {@code V1.equals(V2)} if and only if {@code
 * Arrays.equals(serialize(V1), serialize(V2))}.
 *
 * <p>Notably, protobufs do not have a canonical serialized representation in general, and should be
 * avoided for this purpose.
 */
public interface Converter<V> {
  /** Converts the given bytes into the corresponding typed value. */
  V deserialize(byte[] bytes);

  /** Converts the given value into the corresponding serialized bytes. */
  byte[] serialize(V value);
}
