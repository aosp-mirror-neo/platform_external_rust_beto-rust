/*
 * Copyright 2023 Google LLC
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

import androidx.annotation.Nullable;
import com.google.android.cooperativecleaner.CooperativeCleaner;
import com.google.android.nearby.presence.rust.credential.CredentialBook;
import com.google.android.nearby.presence.rust.credential.V0BroadcastCredential;

/**
 * The main entrypoint to the library.
 *
 * <h3>Supported Features:</h3>
 *
 * <ul>
 *   <li>Create credential books: {@link #newCredentialBookBuilder()}
 *   <li>Deserialize advertisements: {@link #deserializeAdvertisement}
 *   <li>Serialize V0 advertisements: {@link #newPublicV0Builder}, {@link #newEncryptedV0Builder}
 *   <li>Serialize V1 advertisements: {@link #newV1Builder}
 * </ul>
 */
public final class NpAdv extends NativeLibrary {

  public static final String DEFAULT_LIBRARY_NAME = "np_java_ffi";

  // This is effectively an injected variable, but without depending on a DI implementation.
  @SuppressWarnings("NonFinalStaticField")
  @Nullable
  private static CooperativeCleaner cleaner = null;

  /** Create an instance of this library with a custom library name. */
  public NpAdv(String libraryName) {
    super(libraryName);
  }

  /** Create an instance of this library with the default library name {@code "np_java_ffi"}. */
  public NpAdv() {
    this(DEFAULT_LIBRARY_NAME);
  }

  /**
   * Deserialize a Nearby Presence advertisement from its service data bytes.
   *
   * @param serviceData The service data bytes. Must have length&lt;256.
   * @param credentialBook The credential book to use to decrypt.
   * @return A result containing the advertisement if it was able to be deserialized.
   */
  public <M extends CredentialBook.MatchedMetadata> DeserializeResult<M> deserializeAdvertisement(
      byte[] serviceData, CredentialBook<M> credentialBook) {
    this.ensureLoaded();

    DeserializeResult<M> result = nativeDeserializeAdvertisement(serviceData, credentialBook);
    if (result == null) {
      result = new DeserializeResult<M>(DeserializeResult.Kind.UNKNOWN_ERROR);
    }
    return result;
  }

  /** Create a {@link CredentialBook} builder. */
  public <M extends CredentialBook.MatchedMetadata>
      CredentialBook.Builder<M> newCredentialBookBuilder() {
    return CredentialBook.<M>builder(this);
  }

  /** Create an empty {@link CredentialBook}. */
  public CredentialBook<CredentialBook.NoMetadata> newEmptyCredentialBook() {
    return CredentialBook.empty(this);
  }

  /** Create an advertisement builder for a public V0 advertisement. */
  public V0AdvertisementBuilder newPublicV0Builder() {
    return V0AdvertisementBuilder.newPublic(this);
  }

  /** Create an advertisement builder for an encrypted V0 advertisement. */
  public V0AdvertisementBuilder newEncryptedV0Builder(
      V0BroadcastCredential credential, byte[] salt) {
    return V0AdvertisementBuilder.newEncrypted(this, credential, salt);
  }

  /** Create an advertisement builder for a V1 advertisement. */
  public V1AdvertisementBuilder newV1Builder() {
    return V1AdvertisementBuilder.newBuilder(this);
  }

  /**
   * Get the currently configured cleaner. If a cleaner is not configured, a new one will be
   * created.
   */
  public static synchronized CooperativeCleaner getCleaner() {
    if (cleaner == null) {
      cleaner = new CooperativeCleaner();
    }
    return cleaner;
  }

  @Nullable
  private static native <M extends CredentialBook.MatchedMetadata>
      DeserializeResult<M> nativeDeserializeAdvertisement(
          byte[] serviceData, CredentialBook<M> credentialBook);
}
