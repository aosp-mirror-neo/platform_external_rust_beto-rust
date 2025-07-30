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

import static com.google.android.nearby.presence.rust.TestData.ALICE_METADATA;
import static com.google.android.nearby.presence.rust.TestData.V0_BROADCAST_CRED;
import static com.google.android.nearby.presence.rust.TestData.V0_CRED;
import static com.google.android.nearby.presence.rust.TestData.V0_ENCRYPTED_ALICE_METADATA;
import static com.google.android.nearby.presence.rust.TestData.V0_PRIVATE;
import static com.google.android.nearby.presence.rust.TestData.V1_ALICE_METADATA;
import static com.google.android.nearby.presence.rust.TestData.V1_BROADCAST_CRED;
import static com.google.android.nearby.presence.rust.TestData.V1_CRED;
import static com.google.android.nearby.presence.rust.TestData.V1_ENCRYPTED_ALICE_METADATA;
import static com.google.android.nearby.presence.rust.TestData.V1_PRIVATE;
import static com.google.common.truth.Truth.assertThat;

import com.google.android.nearby.presence.rust.credential.CredentialBook;
import com.google.android.nearby.presence.rust.credential.V0BroadcastCredential;
import com.google.android.nearby.presence.rust.credential.V1BroadcastCredential;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;

@RunWith(JUnit4.class)
public class DecryptTests {

  private final NpAdv npAdv = new NpAdv();

  public static final class TestMetadata implements CredentialBook.MatchedMetadata {
    public byte[] plaintextMetadata;
    public byte[] encryptedMetadata;

    public TestMetadata(byte[] plaintextMetadata, byte[] encryptedMetadata) {
      this.plaintextMetadata = plaintextMetadata;
      this.encryptedMetadata = encryptedMetadata;
    }

    @Override
    public byte[] getEncryptedMetadataBytes() {
      return encryptedMetadata;
    }
  }

  public static final TestMetadata V0_METADATA =
      new TestMetadata(ALICE_METADATA, V0_ENCRYPTED_ALICE_METADATA);
  public static final TestMetadata V1_METADATA =
      new TestMetadata(ALICE_METADATA, V1_ENCRYPTED_ALICE_METADATA);

  DeserializeResult<TestMetadata> parsePrivateAdv(byte[] bytes) {
    try (CredentialBook<TestMetadata> book =
        npAdv
            .<TestMetadata>newCredentialBookBuilder()
            .addDiscoveryCredential(V0_CRED, V0_METADATA)
            .addDiscoveryCredential(V1_CRED, V1_METADATA)
            .build()) {
      return npAdv.deserializeAdvertisement(bytes, book);
    }
  }

  @Test
  public void deserializeAdvertisement_v0_canParsePrivate() {
    try (DeserializeResult<TestMetadata> result = parsePrivateAdv(V0_PRIVATE)) {
      assertThat(result.getKind()).isEqualTo(DeserializeResult.Kind.V0_ADVERTISEMENT);

      DeserializedV0Advertisement<TestMetadata> adv = result.getAsV0();

      assertThat(adv).isNotNull();
      assertThat(adv.isLegible()).isTrue();
      assertThat(adv.getIdentity()).isEqualTo(IdentityKind.DECRYPTED);
      assertThat(adv.getMatchedMetadata()).isSameInstanceAs(V0_METADATA);
      assertThat(adv.getDecryptedMetadata()).isEqualTo(ALICE_METADATA);
    }
  }

  @Test
  public void deserializeAdvertisement_v1_canParsePrivate() {
    try (DeserializeResult<TestMetadata> result = parsePrivateAdv(V1_PRIVATE)) {
      assertThat(result.getKind()).isEqualTo(DeserializeResult.Kind.V1_ADVERTISEMENT);

      DeserializedV1Advertisement<TestMetadata> adv = result.getAsV1();

      assertThat(adv).isNotNull();
      assertThat(adv.getNumLegibleSections()).isEqualTo(1);
      assertThat(adv.getNumUndecryptableSections()).isEqualTo(0);

      DeserializedV1Section<TestMetadata> section = adv.getSection(0);
      assertThat(section.getIdentityKind()).isEqualTo(IdentityKind.DECRYPTED);
      assertThat(section.getMatchedMetadata()).isSameInstanceAs(V1_METADATA);
      assertThat(section.getDecryptedMetadata()).isEqualTo(V1_ALICE_METADATA);
    }
  }

  @Test
  public void deserializeAdvertisement_v0_canDecryptEncryptedMetadata() {
    byte[] encryptedMetadata =
        V0BroadcastCredential.encryptMetadata(npAdv, V0_BROADCAST_CRED, ALICE_METADATA);
    TestMetadata v0Metadata = new TestMetadata(ALICE_METADATA, encryptedMetadata);
    try (CredentialBook<TestMetadata> book =
        npAdv
            .<TestMetadata>newCredentialBookBuilder()
            .addDiscoveryCredential(V0_CRED, v0Metadata)
            .build()) {
      DeserializeResult<TestMetadata> result = npAdv.deserializeAdvertisement(V0_PRIVATE, book);

      assertThat(result.getKind()).isEqualTo(DeserializeResult.Kind.V0_ADVERTISEMENT);

      DeserializedV0Advertisement<TestMetadata> adv = result.getAsV0();

      assertThat(adv).isNotNull();
      assertThat(adv.isLegible()).isTrue();
      assertThat(adv.getIdentity()).isEqualTo(IdentityKind.DECRYPTED);
      assertThat(adv.getMatchedMetadata()).isSameInstanceAs(v0Metadata);
      assertThat(adv.getDecryptedMetadata()).isEqualTo(ALICE_METADATA);
    }
  }

  @Test
  public void deserializeAdvertisement_v1_canDecryptEncryptedMetadata() {
    byte[] encryptedMetadata =
        V1BroadcastCredential.encryptMetadata(npAdv, V1_BROADCAST_CRED, ALICE_METADATA);
    TestMetadata v1Metadata = new TestMetadata(ALICE_METADATA, encryptedMetadata);
    try (CredentialBook<TestMetadata> book =
        npAdv
            .<TestMetadata>newCredentialBookBuilder()
            .addDiscoveryCredential(V1_CRED, v1Metadata)
            .build()) {
      DeserializeResult<TestMetadata> result = npAdv.deserializeAdvertisement(V1_PRIVATE, book);

      assertThat(result.getKind()).isEqualTo(DeserializeResult.Kind.V1_ADVERTISEMENT);

      DeserializedV1Advertisement<TestMetadata> adv = result.getAsV1();

      assertThat(adv).isNotNull();
      assertThat(adv.getNumLegibleSections()).isEqualTo(1);
      assertThat(adv.getNumUndecryptableSections()).isEqualTo(0);

      DeserializedV1Section<TestMetadata> section = adv.getSection(0);
      assertThat(section.getIdentityKind()).isEqualTo(IdentityKind.DECRYPTED);
      assertThat(section.getMatchedMetadata()).isSameInstanceAs(v1Metadata);
      assertThat(section.getDecryptedMetadata()).isEqualTo(ALICE_METADATA);
    }
  }
}
