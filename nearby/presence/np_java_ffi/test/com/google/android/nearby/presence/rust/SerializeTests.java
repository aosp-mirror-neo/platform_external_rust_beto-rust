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

import static com.google.android.nearby.presence.rust.TestData.V0_BROADCAST_CRED;
import static com.google.android.nearby.presence.rust.TestData.V0_CRED;
import static com.google.android.nearby.presence.rust.TestData.V1_BROADCAST_CRED;
import static com.google.android.nearby.presence.rust.TestData.V1_CRED;
import static com.google.android.nearby.presence.rust.TestData.V1_PRIVATE;
import static com.google.android.nearby.presence.rust.TestData.V1_PUBLIC;
import static com.google.common.truth.Truth.assertThat;
import static org.junit.Assert.assertThrows;

import com.google.android.nearby.presence.rust.SerializationException.DuplicateDataElementTypeCodeException;
import com.google.android.nearby.presence.rust.V0DataElement.TxPower;
import com.google.android.nearby.presence.rust.V0DataElement.V0ActionType;
import com.google.android.nearby.presence.rust.V0DataElement.V0Actions;
import com.google.android.nearby.presence.rust.V1DataElement.Generic;
import com.google.android.nearby.presence.rust.credential.CredentialBook;
import com.google.android.nearby.presence.rust.credential.CredentialBook.NoMetadata;
import org.junit.Ignore;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.junit.runners.JUnit4;

@RunWith(JUnit4.class)
public class SerializeTests {

  private static final NpAdv npAdv = new NpAdv();

  public static final TxPower TX_POWER = new TxPower(npAdv, 7);
  public static final TxPower INVALID_TX_POWER = new TxPower(npAdv, -777);

  public static final V0Actions PUBLIC_ACTIONS =
      new V0Actions(
          npAdv, IdentityKind.PLAINTEXT, V0ActionType.NEARBY_SHARE, V0ActionType.CROSS_DEV_SDK);
  public static final V0Actions PRIVATE_ACTIONS =
      new V0Actions(
          npAdv, IdentityKind.DECRYPTED, V0ActionType.CALL_TRANSFER, V0ActionType.NEARBY_SHARE);

  public static final Generic GENERIC_DE =
      new Generic(npAdv, 1234, new byte[] {0x01, 0x02, 0x03, 0x04});

  @SuppressWarnings("MutablePublicArray")
  public static final byte[] SALT = new byte[] {0x12, 0x34};

  @Test
  public void constructActionsDe_mergedValueIsNonZero() {
    assertThat(PUBLIC_ACTIONS.getActionBits()).isNotEqualTo(0);
    assertThat(PRIVATE_ACTIONS.getActionBits()).isNotEqualTo(0);
  }

  @Test
  public void serializeAdvertisement_v1_canCreatePubSection() throws Exception {
    try (V1AdvertisementBuilder builder = npAdv.newV1Builder()) {
      try (V1SectionBuilder sectionBuilder = builder.addPublicSection()) {
        sectionBuilder.addDataElement(GENERIC_DE);
      }
      byte[] adv = builder.build();

      assertThat(adv).isNotNull();
      assertThat(adv).isNotEmpty();
    }
  }

  @Test
  public void serializeAdvertisement_v1_canCreateMicEncryptedSection() throws Exception {
    try (V1AdvertisementBuilder builder = npAdv.newV1Builder()) {
      try (V1SectionBuilder sectionBuilder =
          builder.addEncryptedSection(V1_BROADCAST_CRED, VerificationMode.MIC)) {
        sectionBuilder.addDataElement(GENERIC_DE);
      }
      byte[] adv = builder.build();

      assertThat(adv).isNotNull();
      assertThat(adv).isNotEmpty();
    }
  }

  @Test
  public void serializeAdvertisement_v1_canCreateEmptyPublicAdvertisement() throws Exception {
    try (V1AdvertisementBuilder builder = npAdv.newV1Builder()) {

      byte[] adv = builder.build();

      assertThat(adv).isNotNull();
    }
  }

  @Test
  public void serializeAdvertisement_v1_canCreateEmptyPublicSection() throws Exception {
    try (V1AdvertisementBuilder builder = npAdv.newV1Builder()) {
      builder.addPublicSection().close();

      byte[] adv = builder.build();

      assertThat(adv).isNotNull();
    }
  }

  @Test(expected = DuplicateDataElementTypeCodeException.class)
  public void serializeAdvertisement_v1_duplicateTypeCodeException() throws Exception {
    try (V1AdvertisementBuilder advBuilder = npAdv.newV1Builder()) {
      try (V1SectionBuilder sectionBuilder = advBuilder.addPublicSection()) {
        byte[] payload = new byte[2];
        V1DataElement.Generic test = new V1DataElement.Generic(npAdv, 123, payload);
        sectionBuilder.addDataElement(test);
        sectionBuilder.addDataElement(test);
      }
    }
  }

  @Test
  public void serializeAdvertisement_v1_canCreateRoundtripEncrypted() throws Exception {
    try (CredentialBook<NoMetadata> book =
            npAdv
                .<NoMetadata>newCredentialBookBuilder()
                .addDiscoveryCredential(V1_CRED, NoMetadata.INSTANCE)
                .build();
        DeserializeResult<NoMetadata> original = npAdv.deserializeAdvertisement(V1_PRIVATE, book);
        V1AdvertisementBuilder builder = npAdv.newV1Builder()) {
      for (DeserializedV1Section<NoMetadata> section : original.getAsV1().getSections()) {
        try (V1SectionBuilder sectionBuilder =
            builder.addEncryptedSection(V1_BROADCAST_CRED, VerificationMode.MIC)) {
          for (V1DataElement.Generic de : section.getDataElements()) {
            sectionBuilder.addDataElement(de);
          }
        }
      }
      byte[] adv = builder.build();

      // Can't check contents due to mismatched salts
      assertThat(adv.length).isEqualTo(V1_PRIVATE.length);
    }
  }

  @Test
  public void serializeAdvertisement_v1_canCreateRoundtripPublic() throws Exception {
    try (CredentialBook<NoMetadata> book = npAdv.newEmptyCredentialBook();
        DeserializeResult<NoMetadata> original = npAdv.deserializeAdvertisement(V1_PUBLIC, book);
        V1AdvertisementBuilder builder = npAdv.newV1Builder()) {
      for (DeserializedV1Section<NoMetadata> section : original.getAsV1().getSections()) {
        try (V1SectionBuilder sectionBuilder = builder.addPublicSection()) {
          for (V1DataElement.Generic de : section.getDataElements()) {
            sectionBuilder.addDataElement(de);
          }
        }
      }
      byte[] adv = builder.build();

      assertThat(adv).isEqualTo(V1_PUBLIC);
    }
  }

  @Test
  public void serializeAdvertisement_v0_canSerialize() throws Exception {
    try (V0AdvertisementBuilder builder = npAdv.newPublicV0Builder()) {
      builder.addDataElement(TX_POWER);
      builder.addDataElement(PUBLIC_ACTIONS);
      byte[] adv = builder.build();
      assertThat(adv).isNotNull();
    }
  }

  @Test
  public void serializeAdvertisement_v0_canSerializePrivate() throws Exception {
    try (V0AdvertisementBuilder builder = npAdv.newEncryptedV0Builder(V0_BROADCAST_CRED, SALT)) {
      builder.addDataElement(TX_POWER);
      builder.addDataElement(PRIVATE_ACTIONS);
      byte[] adv = builder.build();
      assertThat(adv).isNotNull();
    }
  }

  @Test
  public void serializeAdvertisement_v0_canRoundtrip() throws Exception {
    try (V0AdvertisementBuilder builder = npAdv.newPublicV0Builder();
        CredentialBook<NoMetadata> book = npAdv.newEmptyCredentialBook()) {
      builder.addDataElement(TX_POWER);
      builder.addDataElement(PUBLIC_ACTIONS);
      byte[] advBytes = builder.build();

      DeserializeResult<NoMetadata> result = npAdv.deserializeAdvertisement(advBytes, book);
      DeserializedV0Advertisement<NoMetadata> adv = result.getAsV0();

      assertThat(adv).isNotNull();
      assertThat(adv.getDataElementCount()).isEqualTo(2);
      V0DataElement de0 = adv.getDataElement(0);
      assertThat(de0).isInstanceOf(TxPower.class);
      TxPower txPower = (TxPower) de0;
      assertThat(txPower.getTxPower()).isEqualTo(TX_POWER.getTxPower());
      V0DataElement de1 = adv.getDataElement(1);
      assertThat(de1).isInstanceOf(V0Actions.class);
      V0Actions v0Actions = (V0Actions) de1;
      assertThat(v0Actions.getIdentityKind()).isEqualTo(IdentityKind.PLAINTEXT);
      assertThat(v0Actions.getActionBits()).isEqualTo(PUBLIC_ACTIONS.getActionBits());
    }
  }

  @Test
  public void serializeAdvertisement_v0_canRoundtripPrivate() throws Exception {
    try (V0AdvertisementBuilder builder = npAdv.newEncryptedV0Builder(V0_BROADCAST_CRED, SALT);
        CredentialBook<NoMetadata> book =
            npAdv
                .<NoMetadata>newCredentialBookBuilder()
                .addDiscoveryCredential(V0_CRED, NoMetadata.INSTANCE)
                .build()) {
      builder.addDataElement(TX_POWER);
      builder.addDataElement(PRIVATE_ACTIONS);
      byte[] advBytes = builder.build();

      DeserializeResult<NoMetadata> result = npAdv.deserializeAdvertisement(advBytes, book);
      DeserializedV0Advertisement<NoMetadata> adv = result.getAsV0();

      assertThat(adv).isNotNull();
      assertThat(adv.getDataElementCount()).isEqualTo(2);
      V0DataElement de0 = adv.getDataElement(0);
      assertThat(de0).isInstanceOf(TxPower.class);
      TxPower txPower = (TxPower) de0;
      assertThat(txPower.getTxPower()).isEqualTo(TX_POWER.getTxPower());
      V0DataElement de1 = adv.getDataElement(1);
      assertThat(de1).isInstanceOf(V0Actions.class);
      V0Actions v0Actions = (V0Actions) de1;
      assertThat(v0Actions.getIdentityKind()).isEqualTo(IdentityKind.DECRYPTED);
      assertThat(v0Actions.getActionBits()).isEqualTo(PRIVATE_ACTIONS.getActionBits());
    }
  }

  @Test
  public void serializeAdvertisement_v0_emptyIsError() throws Exception {
    try (V0AdvertisementBuilder builder = npAdv.newPublicV0Builder()) {
      assertThrows(SerializationException.UnencryptedSizeException.class, () -> builder.build());
    }
  }

  @Test
  public void serializeAdvertisement_v0_fullIsError() throws Exception {
    try (V0AdvertisementBuilder builder = npAdv.newPublicV0Builder()) {
      assertThrows(
          SerializationException.InsufficientSpaceException.class,
          () -> {
            // Overfill the advertisement. Fixing b/311225033 will break this method.
            for (int i = 0; i < 50; i++) {
              builder.addDataElement(TX_POWER);
            }
          });
    }
  }

  @Test
  public void serializeAdvertisement_v0_publicAdvPrivateActionsIsError() throws Exception {
    try (V0AdvertisementBuilder builder = npAdv.newPublicV0Builder()) {
      assertThrows(
          SerializationException.InvalidDataElementException.class,
          () -> builder.addDataElement(PRIVATE_ACTIONS));
    }
  }

  @Test
  public void serializeAdvertisement_v0_privateAdvPublicActionsIsError() throws Exception {
    try (V0AdvertisementBuilder builder = npAdv.newEncryptedV0Builder(V0_BROADCAST_CRED, SALT)) {
      assertThrows(
          SerializationException.InvalidDataElementException.class,
          () -> builder.addDataElement(PUBLIC_ACTIONS));
    }
  }

  @Test
  public void serializeAdvertisement_v0_invalidTxPowerIsError() throws Exception {
    try (V0AdvertisementBuilder builder = npAdv.newPublicV0Builder()) {
      assertThrows(
          SerializationException.InvalidDataElementException.class,
          () -> builder.addDataElement(INVALID_TX_POWER));
    }
  }

  @Test
  public void serializeAdvertisement_v0_handleIsConsumedByBuild() throws Exception {
    try (V0AdvertisementBuilder builder = npAdv.newPublicV0Builder()) {
      builder.addDataElement(TX_POWER);
      byte[] unused = builder.build();
      assertThrows(Handle.InvalidHandleException.class, () -> builder.addDataElement(TX_POWER));
      assertThrows(Handle.InvalidHandleException.class, () -> builder.build());
    }
  }

  @Test
  @Ignore("b/311225033: Duplicate data element spec change not implemented")
  public void serializeAdvertisement_v0_deNotAddedTwice() throws Exception {
    try (V0AdvertisementBuilder builder = npAdv.newPublicV0Builder()) {
      builder.addDataElement(TX_POWER);
      assertThrows(Exception.class, () -> builder.addDataElement(TX_POWER));
    }
  }
}
