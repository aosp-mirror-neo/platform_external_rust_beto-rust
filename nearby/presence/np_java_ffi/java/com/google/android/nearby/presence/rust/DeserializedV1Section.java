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
import com.google.android.nearby.presence.rust.credential.CredentialBook;
import java.util.Iterator;

/**
 * A section from a deserialized V1 advertisement. This object is valid for only as long as the
 * containing advertisement object is valid; it is backed by the same {@link LegibleV1Sections}
 * instance.
 */
@UsedByNative
public final class DeserializedV1Section<M extends CredentialBook.MatchedMetadata> {

  private final LegibleV1Sections legibleSections;
  private final int legibleSectionsIndex;
  private final int numDataElements;
  @IdentityKind private final int identityTag;
  private final CredentialBook<M> credentialBook;

  @UsedByNative
  /* package */ DeserializedV1Section(
      LegibleV1Sections legibleSections,
      int legibleSectionsIndex,
      int numDataElements,
      @IdentityKind int identityTag,
      CredentialBook<M> credentialBook) {
    this.legibleSections = legibleSections;
    this.legibleSectionsIndex = legibleSectionsIndex;
    this.numDataElements = numDataElements;
    this.identityTag = identityTag;
    this.credentialBook = credentialBook;
  }

  /** Gets the identity kind for this section. */
  @IdentityKind
  public int getIdentityKind() {
    return this.identityTag;
  }

  /** Gets the number of data elements in this section. */
  public int getDataElementCount() {
    return this.numDataElements;
  }

  /**
   * Gets the data element at the given {@code index} in this advertisement.
   *
   * @throws IndexOutOfBoundsException if the index is invalid
   */
  public V1DataElement getDataElement(int index) {
    return legibleSections.getSectionDataElement(this.legibleSectionsIndex, index);
  }

  /** Gets all the data elements for iteration. */
  public Iterable<V1DataElement.Generic> getDataElements() {
    return () -> new DataElementIterator(legibleSections, legibleSectionsIndex, numDataElements);
  }

  /**
   * Gets the identity token for the credential this advertisement was deserialized with. This will
   * return {@code null} if this advertisement is not an encrypted advertisement.
   */
  @Nullable
  public byte[] getIdentityToken() {
    LegibleV1Sections.IdentityDetails details =
        legibleSections.getSectionIdentityDetails(legibleSectionsIndex);
    return (details != null) ? details.getIdentityToken() : null;
  }

  /**
   * Gets the key seed for the credential this advertisement was decrypted with. This will return
   * {@code null} if this advertisement is not an encrypted advertisement.
   */
  @Nullable
  public byte[] getKeySeed() {
    LegibleV1Sections.IdentityDetails details =
        legibleSections.getSectionIdentityDetails(legibleSectionsIndex);
    return (details != null) ? details.getKeySeed() : null;
  }

  /**
   * Gets the verification mode for this advertisement. This will return {@code null} if this
   * advertisement is not an encrypted advertisement.
   */
  @Nullable
  @VerificationMode
  public Integer getVerificationMode() {
    LegibleV1Sections.IdentityDetails details =
        legibleSections.getSectionIdentityDetails(legibleSectionsIndex);
    return (details != null) ? details.getVerificationMode() : null;
  }

  /**
   * Gets the metadata matched to the credential this advertisement was deserialized with. This will
   * return {@code null} if this advertisement is not an encrypted advertisement.
   */
  @Nullable
  public M getMatchedMetadata() {
    LegibleV1Sections.IdentityDetails details =
        legibleSections.getSectionIdentityDetails(legibleSectionsIndex);
    if (details == null) {
      return null;
    }
    return credentialBook.getMatchedMetadata(details.getCredentialId());
  }

  /**
   * Gets the metadata matched to the credential this advertisement was deserialized with. This will
   * return {@code null} if this advertisement is not an encrypted advertisement or the metadata was
   * not able to be decrypted.
   */
  @Nullable
  public byte[] getDecryptedMetadata() {
    return legibleSections.getSectionDecryptedMetadata(legibleSectionsIndex);
  }

  private static final class DataElementIterator implements Iterator<V1DataElement.Generic> {
    private final LegibleV1Sections legibleSections;
    private final int legibleSectionsIndex;
    private final int numDataElements;

    private int position = 0;

    public DataElementIterator(
        LegibleV1Sections legibleSections, int legibleSectionsIndex, int numDataElements) {
      this.legibleSections = legibleSections;
      this.legibleSectionsIndex = legibleSectionsIndex;
      this.numDataElements = numDataElements;
    }

    @Override
    public boolean hasNext() {
      return position < numDataElements;
    }

    @Override
    public V1DataElement.Generic next() {
      return legibleSections.getSectionDataElement(legibleSectionsIndex, position++);
    }
  }
}
